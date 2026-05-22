// Memory Safety Module for pblang
// Implements Rust-like ownership, borrowing, and lifetime tracking

use pblang_ir::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Represents a unique identifier for each memory location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryId(pub usize);

/// Represents a unique identifier for each borrow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BorrowId(pub usize);

/// Represents a borrow with its associated lifetime and permissions
#[derive(Debug, Clone)]
pub struct Borrow {
    pub id: BorrowId,
    pub memory_id: MemoryId,
    pub borrow_type: BorrowType,
    pub lifetime: LifetimeId,
    pub span: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BorrowType {
    Immutable, // &T
    Mutable,   // &mut T
    Unique,    // &move T (consumes the original)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifetimeId(pub usize);

/// Error type for memory safety violations.
///
/// Replaces the previous `String`-based errors with structured,
/// programmatic error variants that can be matched on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySafetyError {
    /// Tried to create a mutable borrow while immutable borrows already exist
    BorrowConflict(MemoryId),
    /// Tried to access a value that has been moved (consumed)
    UseAfterMove(MemoryId),
    /// Tried to access a value whose lifetime has ended
    LifetimeExpired(MemoryId),
    /// Tried to create any borrow while a mutable borrow already exists
    AlreadyBorrowed(MemoryId),
    /// Tried to access a memory location that was never allocated
    NotAllocated(MemoryId),
    /// Tried to mutate an immutable value
    Immutable(MemoryId),
    /// Tried to deallocate a value that is still borrowed
    CannotDeallocateBorrowed(MemoryId),
    /// Tried to pop the root lifetime scope
    CannotPopRootLifetime,
    /// No borrow of the specified type was found for release
    BorrowNotFound(MemoryId, BorrowType),
}

impl fmt::Display for MemorySafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemorySafetyError::BorrowConflict(id) => {
                write!(
                    f,
                    "Cannot create mutable borrow at {:?}: immutable borrows already exist",
                    id
                )
            }
            MemorySafetyError::UseAfterMove(id) => {
                write!(f, "Use after move: value at {:?} has been consumed", id)
            }
            MemorySafetyError::LifetimeExpired(id) => {
                write!(f, "Lifetime expired: value at {:?} is no longer valid", id)
            }
            MemorySafetyError::AlreadyBorrowed(id) => {
                write!(
                    f,
                    "Cannot borrow at {:?}: mutable borrow already exists",
                    id
                )
            }
            MemorySafetyError::NotAllocated(id) => {
                write!(f, "Memory location {:?} not allocated", id)
            }
            MemorySafetyError::Immutable(id) => {
                write!(f, "Cannot mutate immutable value at {:?}", id)
            }
            MemorySafetyError::CannotDeallocateBorrowed(id) => {
                write!(f, "Cannot deallocate borrowed value at {:?}", id)
            }
            MemorySafetyError::CannotPopRootLifetime => {
                write!(f, "Cannot pop root lifetime")
            }
            MemorySafetyError::BorrowNotFound(id, bt) => {
                write!(f, "No {:?} borrow found for memory location {:?}", bt, id)
            }
        }
    }
}

impl std::error::Error for MemorySafetyError {}

/// Tracks the ownership state of a single memory location.
#[derive(Debug)]
struct OwnerInfo {
    pub is_mutable: bool,
    #[allow(dead_code)]
    pub current_lifetime: LifetimeId,
    pub is_borrowed: bool,
    pub is_consumed: bool,
    pub is_lifetime_expired: bool,
    #[allow(dead_code)]
    pub original_value: Option<Value>,
}

/// Inner mutable state shared between `MemoryTracker` and `SafeValue`
/// via `Rc<RefCell<>>`. This enables `SafeValue`'s `Drop` implementation
/// to release borrows without requiring a direct reference to the
/// `MemoryTracker`.
pub(crate) struct TrackerInner {
    owners: HashMap<MemoryId, OwnerInfo>,
    borrows: HashMap<MemoryId, Vec<Borrow>>,
    lifetime_stack: Vec<LifetimeId>,
    /// Maps each lifetime to the MemoryIds allocated within it.
    lifetime_values: HashMap<LifetimeId, Vec<MemoryId>>,
    /// Global value store (replaces the previous Arc<RwLock<GlobalMemoryState>>).
    values: HashMap<MemoryId, Value>,
    next_memory_id: usize,
    next_lifetime_id: usize,
    next_borrow_id: usize,
}

impl fmt::Debug for TrackerInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrackerInner")
            .field("owners_count", &self.owners.len())
            .field(
                "borrows_count",
                &self.borrows.values().map(|v| v.len()).sum::<usize>(),
            )
            .field("lifetime_depth", &self.lifetime_stack.len())
            .field("values_count", &self.values.len())
            .field("next_memory_id", &self.next_memory_id)
            .field("next_borrow_id", &self.next_borrow_id)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// MemoryTracker
// ---------------------------------------------------------------------------

/// Tracks the ownership and borrowing state of memory locations,
/// enforcing Rust-like borrow rules at runtime.
///
/// **Borrow rules enforced:**
/// - At most one `Mutable` borrow at a time, with no `Immutable` borrows coexisting.
/// - Multiple `Immutable` borrows are allowed simultaneously.
/// - A `Unique` borrow (move) consumes the value, preventing further access.
/// - Lifetime scoping: when a lifetime is popped, all borrows and values
///   created within it are invalidated.
/// - Use-after-move detection: `get()` and `set()` check `is_consumed`.
pub struct MemoryTracker {
    inner: Rc<RefCell<TrackerInner>>,
    /// Global state retained for backward compatibility with any external
    /// code that might read `global_state` (not used internally anymore).
    global_state: Rc<RefCell<GlobalMemoryState>>,
}

#[derive(Debug, Default)]
struct GlobalMemoryState {
    all_locations: HashMap<MemoryId, MemoryLocation>,
    ref_counts: HashMap<MemoryId, usize>,
}

#[derive(Debug)]
struct MemoryLocation {
    pub value: Value,
    #[allow(dead_code)]
    pub is_mutable: bool,
    #[allow(dead_code)]
    pub created_at_lifetime: LifetimeId,
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryTracker {
    pub fn new() -> Self {
        let inner = TrackerInner {
            owners: HashMap::new(),
            borrows: HashMap::new(),
            lifetime_stack: vec![LifetimeId(0)],
            lifetime_values: HashMap::new(),
            values: HashMap::new(),
            next_memory_id: 1,
            next_lifetime_id: 1,
            next_borrow_id: 1,
        };
        Self {
            inner: Rc::new(RefCell::new(inner)),
            global_state: Rc::new(RefCell::new(GlobalMemoryState::default())),
        }
    }

    // -----------------------------------------------------------------------
    // Lifetime management
    // -----------------------------------------------------------------------

    /// Create a new lifetime scope and push it onto the stack.
    pub fn push_lifetime(&mut self) -> LifetimeId {
        let mut inner = self.inner.borrow_mut();
        let lifetime = LifetimeId(inner.next_lifetime_id);
        inner.next_lifetime_id += 1;
        inner.lifetime_stack.push(lifetime);
        inner.lifetime_values.insert(lifetime, Vec::new());
        lifetime
    }

    /// Exit the current lifetime scope.
    ///
    /// All borrows created within this lifetime are released, and all
    /// values allocated within this lifetime are marked as expired.
    /// Subsequent access to those values returns `MemorySafetyError::LifetimeExpired`.
    pub fn pop_lifetime(&mut self) -> Result<LifetimeId, MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();
        if inner.lifetime_stack.len() <= 1 {
            return Err(MemorySafetyError::CannotPopRootLifetime);
        }

        let expired = inner.lifetime_stack.pop().expect("just checked len > 1");

        // Mark all values allocated in this lifetime as expired.
        if let Some(ids) = inner.lifetime_values.remove(&expired) {
            for &mid in &ids {
                if let Some(owner) = inner.owners.get_mut(&mid) {
                    owner.is_lifetime_expired = true;
                }
            }
        }

        // Remove all borrows whose lifetime matches the expired one.
        // Phase 1: remove expired borrows and collect mids whose list became empty.
        let mut empty_mids: Vec<MemoryId> = Vec::new();
        for (mid, borrows) in inner.borrows.iter_mut() {
            borrows.retain(|b| b.lifetime != expired);
            if borrows.is_empty() {
                empty_mids.push(*mid);
            }
        }
        // Phase 2: update owner state for emptied borrow lists.
        for mid in empty_mids {
            if let Some(owner) = inner.owners.get_mut(&mid) {
                owner.is_borrowed = false;
            }
        }

        Ok(expired)
    }

    /// Get the current lifetime.
    pub fn current_lifetime(&self) -> LifetimeId {
        self.inner
            .borrow()
            .lifetime_stack
            .last()
            .copied()
            .unwrap_or(LifetimeId(0))
    }

    // -----------------------------------------------------------------------
    // Allocation
    // -----------------------------------------------------------------------

    /// Allocate a new memory location with ownership.
    pub fn allocate(&mut self, value: Value, is_mutable: bool) -> MemoryId {
        let mut inner = self.inner.borrow_mut();
        let memory_id = MemoryId(inner.next_memory_id);
        inner.next_memory_id += 1;
        let current_lifetime = *inner.lifetime_stack.last().unwrap_or(&LifetimeId(0));

        inner.owners.insert(
            memory_id,
            OwnerInfo {
                is_mutable,
                current_lifetime,
                is_borrowed: false,
                is_consumed: false,
                is_lifetime_expired: false,
                original_value: Some(value.clone()),
            },
        );

        // Track which lifetime this value was allocated in.
        inner
            .lifetime_values
            .entry(current_lifetime)
            .or_default()
            .push(memory_id);

        // Store the value in the inner map.
        inner.values.insert(memory_id, value.clone());

        // Mirror into global_state for backward compatibility.
        {
            let mut global = self.global_state.borrow_mut();
            global.all_locations.insert(
                memory_id,
                MemoryLocation {
                    value: value.clone(),
                    is_mutable,
                    created_at_lifetime: current_lifetime,
                },
            );
            global.ref_counts.insert(memory_id, 1);
        }

        memory_id
    }

    // -----------------------------------------------------------------------
    // Borrowing
    // -----------------------------------------------------------------------

    /// Create an immutable borrow of a memory location.
    ///
    /// **Enforces:** No mutable borrows may exist on `memory_id`.
    pub fn borrow_immutable(
        &mut self,
        memory_id: MemoryId,
        span: Option<usize>,
    ) -> Result<BorrowId, MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();
        self.validate_borrow_inner(&inner, memory_id, BorrowType::Immutable)?;

        let borrow_id = BorrowId(inner.next_borrow_id);
        inner.next_borrow_id += 1;

        let borrow = Borrow {
            id: borrow_id,
            memory_id,
            borrow_type: BorrowType::Immutable,
            lifetime: *inner.lifetime_stack.last().unwrap_or(&LifetimeId(0)),
            span,
        };

        inner.borrows.entry(memory_id).or_default().push(borrow);

        if let Some(owner) = inner.owners.get_mut(&memory_id) {
            owner.is_borrowed = true;
        }

        Ok(borrow_id)
    }

    /// Create a mutable borrow of a memory location.
    ///
    /// **Enforces:** No other borrows (mutable or immutable) may exist.
    pub fn borrow_mutable(
        &mut self,
        memory_id: MemoryId,
        span: Option<usize>,
    ) -> Result<BorrowId, MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();
        self.validate_borrow_inner(&inner, memory_id, BorrowType::Mutable)?;

        let borrow_id = BorrowId(inner.next_borrow_id);
        inner.next_borrow_id += 1;

        let borrow = Borrow {
            id: borrow_id,
            memory_id,
            borrow_type: BorrowType::Mutable,
            lifetime: *inner.lifetime_stack.last().unwrap_or(&LifetimeId(0)),
            span,
        };

        inner.borrows.entry(memory_id).or_default().push(borrow);

        if let Some(owner) = inner.owners.get_mut(&memory_id) {
            owner.is_borrowed = true;
        }

        Ok(borrow_id)
    }

    /// Create a unique borrow (move/consume).
    ///
    /// **Enforces:** No other borrows may exist, and the value must not
    /// already be consumed.
    pub fn borrow_unique(
        &mut self,
        memory_id: MemoryId,
        span: Option<usize>,
    ) -> Result<BorrowId, MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();
        self.validate_borrow_inner(&inner, memory_id, BorrowType::Unique)?;

        let borrow_id = BorrowId(inner.next_borrow_id);
        inner.next_borrow_id += 1;

        let borrow = Borrow {
            id: borrow_id,
            memory_id,
            borrow_type: BorrowType::Unique,
            lifetime: *inner.lifetime_stack.last().unwrap_or(&LifetimeId(0)),
            span,
        };

        inner.borrows.entry(memory_id).or_default().push(borrow);

        if let Some(owner) = inner.owners.get_mut(&memory_id) {
            owner.is_consumed = true;
            owner.is_borrowed = true;
        }

        Ok(borrow_id)
    }

    /// Validate that a borrow is allowed according to Rust borrowing rules.
    fn validate_borrow_inner(
        &self,
        inner: &TrackerInner,
        memory_id: MemoryId,
        borrow_type: BorrowType,
    ) -> Result<(), MemorySafetyError> {
        let owner = inner
            .owners
            .get(&memory_id)
            .ok_or(MemorySafetyError::NotAllocated(memory_id))?;

        // Check use-after-move
        if owner.is_consumed {
            return Err(MemorySafetyError::UseAfterMove(memory_id));
        }

        // Check lifetime expiry
        if owner.is_lifetime_expired {
            return Err(MemorySafetyError::LifetimeExpired(memory_id));
        }

        // Check existing borrows
        if let Some(existing) = inner.borrows.get(&memory_id) {
            for eb in existing {
                // If any existing borrow's lifetime has expired, skip it
                // (shouldn't happen after cleanup, but be defensive)
                if inner
                    .owners
                    .get(&eb.memory_id)
                    .is_some_and(|o| o.is_lifetime_expired)
                {
                    continue;
                }

                match (&eb.borrow_type, &borrow_type) {
                    // Existing mutable → no new borrows allowed at all
                    (BorrowType::Mutable, _) | (_, BorrowType::Mutable) => {
                        return Err(MemorySafetyError::AlreadyBorrowed(memory_id));
                    }
                    // Existing unique or new unique → no other borrows
                    (BorrowType::Unique, _) | (_, BorrowType::Unique) => {
                        return Err(MemorySafetyError::AlreadyBorrowed(memory_id));
                    }
                    // Immutable + Immutable is fine
                    (BorrowType::Immutable, BorrowType::Immutable) => {}
                }
            }
        }

        // Cannot borrow mutably (Mutable or Unique) an immutable value
        if matches!(borrow_type, BorrowType::Mutable | BorrowType::Unique) && !owner.is_mutable {
            return Err(MemorySafetyError::Immutable(memory_id));
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Borrow release
    // -----------------------------------------------------------------------

    /// Release a specific borrow by its `BorrowId`.
    ///
    /// When a `SafeValue` (representing a borrow) is dropped, it calls this
    /// method to remove its borrow from the tracking list.
    pub fn release_borrow_by_id(&mut self, borrow_id: BorrowId) -> Result<(), MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();

        // Find the borrow across all memory locations
        let mut found_mid: Option<MemoryId> = None;
        let mut found_idx: Option<usize> = None;

        for (mid, borrows) in inner.borrows.iter() {
            if let Some(idx) = borrows.iter().position(|b| b.id == borrow_id) {
                found_mid = Some(*mid);
                found_idx = Some(idx);
                break;
            }
        }

        let mid = match found_mid {
            Some(m) => m,
            None => return Ok(()), // borrow already cleaned up (e.g. lifetime expired)
        };
        let idx = match found_idx {
            Some(i) => i,
            None => return Ok(()),
        };

        // Remove the borrow entry
        if let Some(borrows) = inner.borrows.get_mut(&mid) {
            borrows.remove(idx);
            // Update owner borrow state
            if borrows.is_empty() {
                if let Some(owner) = inner.owners.get_mut(&mid) {
                    owner.is_borrowed = false;
                }
            }
        }

        Ok(())
    }

    /// Release a borrow by `MemoryId` and `BorrowType`.
    ///
    /// Removes the **first** matching borrow. For more precise control,
    /// prefer `release_borrow_by_id`.
    pub fn release_borrow(
        &mut self,
        memory_id: MemoryId,
        borrow_type: BorrowType,
    ) -> Result<(), MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();

        let borrows =
            inner
                .borrows
                .get_mut(&memory_id)
                .ok_or(MemorySafetyError::BorrowNotFound(
                    memory_id,
                    borrow_type.clone(),
                ))?;

        let idx = borrows
            .iter()
            .position(|b| b.borrow_type == borrow_type)
            .ok_or(MemorySafetyError::BorrowNotFound(
                memory_id,
                borrow_type.clone(),
            ))?;

        borrows.remove(idx);

        if borrows.is_empty() {
            if let Some(owner) = inner.owners.get_mut(&memory_id) {
                owner.is_borrowed = false;
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Read / Write access
    // -----------------------------------------------------------------------

    /// Get the value at a memory location (with borrow and lifetime checking).
    pub fn get(&self, memory_id: MemoryId) -> Result<Value, MemorySafetyError> {
        let inner = self.inner.borrow();

        let owner = inner
            .owners
            .get(&memory_id)
            .ok_or(MemorySafetyError::NotAllocated(memory_id))?;

        if owner.is_consumed {
            return Err(MemorySafetyError::UseAfterMove(memory_id));
        }

        if owner.is_lifetime_expired {
            return Err(MemorySafetyError::LifetimeExpired(memory_id));
        }

        inner
            .values
            .get(&memory_id)
            .cloned()
            .ok_or(MemorySafetyError::NotAllocated(memory_id))
    }

    /// Update the value at a memory location (requires mutable, non-consumed, non-expired).
    pub fn set(&mut self, memory_id: MemoryId, new_value: Value) -> Result<(), MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();

        let owner = inner
            .owners
            .get(&memory_id)
            .ok_or(MemorySafetyError::NotAllocated(memory_id))?;

        if !owner.is_mutable {
            return Err(MemorySafetyError::Immutable(memory_id));
        }
        if owner.is_consumed {
            return Err(MemorySafetyError::UseAfterMove(memory_id));
        }
        if owner.is_lifetime_expired {
            return Err(MemorySafetyError::LifetimeExpired(memory_id));
        }

        // Exclusive mutation: reject while any active borrows exist.
        if let Some(borrows) = inner.borrows.get(&memory_id) {
            if !borrows.is_empty() {
                return Err(MemorySafetyError::AlreadyBorrowed(memory_id));
            }
        }

        // Update value
        inner.values.insert(memory_id, new_value.clone());

        // Update original_value in owner info
        if let Some(owner) = inner.owners.get_mut(&memory_id) {
            owner.original_value = Some(new_value.clone());
        }

        // Mirror into global_state for backward compatibility
        {
            let mut global = self.global_state.borrow_mut();
            if let Some(loc) = global.all_locations.get_mut(&memory_id) {
                loc.value = new_value;
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Deallocation
    // -----------------------------------------------------------------------

    /// Deallocate a memory location (similar to Drop in Rust).
    ///
    /// **Enforces:** The value must not be currently borrowed.
    pub fn deallocate(&mut self, memory_id: MemoryId) -> Result<(), MemorySafetyError> {
        let mut inner = self.inner.borrow_mut();

        // Check for active borrows
        if let Some(borrows) = inner.borrows.get(&memory_id) {
            if !borrows.is_empty() {
                return Err(MemorySafetyError::CannotDeallocateBorrowed(memory_id));
            }
        }

        // Remove from owners and values
        inner.owners.remove(&memory_id);
        inner.values.remove(&memory_id);
        inner.borrows.remove(&memory_id);

        // Remove from lifetime_values tracking
        for vals in inner.lifetime_values.values_mut() {
            vals.retain(|&id| id != memory_id);
        }

        // Mirror into global_state
        {
            let mut global = self.global_state.borrow_mut();
            global.all_locations.remove(&memory_id);
            global.ref_counts.remove(&memory_id);
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Introspection
    // -----------------------------------------------------------------------

    /// Get memory safety statistics for debugging.
    pub fn get_stats(&self) -> MemorySafetyStats {
        let inner = self.inner.borrow();
        let global = self.global_state.borrow();
        MemorySafetyStats {
            total_locations: inner.owners.len(),
            active_borrows: inner.borrows.values().map(|v| v.len()).sum(),
            current_lifetime_depth: inner.lifetime_stack.len(),
            ref_count_total: global.ref_counts.values().sum(),
        }
    }

    /// Check whether a memory location is currently consumed (moved).
    pub fn is_consumed(&self, memory_id: MemoryId) -> bool {
        self.inner
            .borrow()
            .owners
            .get(&memory_id)
            .is_some_and(|o| o.is_consumed)
    }

    /// Check whether a memory location's lifetime has expired.
    pub fn is_lifetime_expired(&self, memory_id: MemoryId) -> bool {
        self.inner
            .borrow()
            .owners
            .get(&memory_id)
            .is_none_or(|o| o.is_lifetime_expired)
    }

    /// Check whether a memory location allows mutable access.
    ///
    /// Returns Ok(()) if the value is allocated, mutable, not consumed,
    /// not lifetime-expired, and has no active borrows. Used by the VM
    /// bytecode loop to guard array/map mutations.
    pub fn check_mutable_allowed(&self, memory_id: MemoryId) -> Result<(), MemorySafetyError> {
        let inner = self.inner.borrow();

        let owner = inner
            .owners
            .get(&memory_id)
            .ok_or(MemorySafetyError::NotAllocated(memory_id))?;

        if !owner.is_mutable {
            return Err(MemorySafetyError::Immutable(memory_id));
        }
        if owner.is_consumed {
            return Err(MemorySafetyError::UseAfterMove(memory_id));
        }
        if owner.is_lifetime_expired {
            return Err(MemorySafetyError::LifetimeExpired(memory_id));
        }
        if let Some(borrows) = inner.borrows.get(&memory_id) {
            if !borrows.is_empty() {
                return Err(MemorySafetyError::AlreadyBorrowed(memory_id));
            }
        }

        Ok(())
    }

    /// Check whether a memory location has any active borrows (of any type).
    pub fn is_borrowed(&self, memory_id: MemoryId) -> bool {
        let inner = self.inner.borrow();
        inner
            .borrows
            .get(&memory_id)
            .is_some_and(|v| !v.is_empty())
    }

    /// Return a reference-counted handle to the inner tracker, allowing
    /// `SafeValue` instances to clean up on Drop.
    pub(crate) fn inner_handle(&self) -> Rc<RefCell<TrackerInner>> {
        Rc::clone(&self.inner)
    }
}

// ---------------------------------------------------------------------------
// SafeValue + Drop
// ---------------------------------------------------------------------------

/// Memory-safe value wrapper that enforces borrowing rules.
///
/// When a `SafeValue` representing a borrow is dropped, its `Drop`
/// implementation automatically releases the borrow from the tracker.
/// Owned values (`BorrowType::Unique`) are *not* auto-released on drop —
/// they must be explicitly deallocated.
#[derive(Debug)]
pub struct SafeValue {
    pub memory_id: MemoryId,
    pub borrow_type: BorrowType,
    pub borrow_id: Option<BorrowId>,
    /// Shared reference to the tracker inner state so that Drop can
    /// release borrows. `None` for owned (non-borrowed) values.
    tracker_inner: Option<Rc<RefCell<TrackerInner>>>,
}

impl SafeValue {
    /// Create a new owned value (not tracked for auto-release on Drop).
    pub fn new(memory_id: MemoryId) -> Self {
        Self {
            memory_id,
            borrow_type: BorrowType::Unique,
            borrow_id: None,
            tracker_inner: None,
        }
    }

    /// Create an immutable reference, associated with the given tracker.
    ///
    /// This does **not** create a borrow in the tracker — it merely
    /// wraps an existing borrow ID for Drop cleanup.
    pub fn borrow_with_tracker(
        memory_id: MemoryId,
        borrow_id: BorrowId,
        tracker: &MemoryTracker,
    ) -> Self {
        Self {
            memory_id,
            borrow_type: BorrowType::Immutable,
            borrow_id: Some(borrow_id),
            tracker_inner: Some(tracker.inner_handle()),
        }
    }

    /// Create a mutable reference, associated with the given tracker.
    pub fn borrow_mut_with_tracker(
        memory_id: MemoryId,
        borrow_id: BorrowId,
        tracker: &MemoryTracker,
    ) -> Self {
        Self {
            memory_id,
            borrow_type: BorrowType::Mutable,
            borrow_id: Some(borrow_id),
            tracker_inner: Some(tracker.inner_handle()),
        }
    }

    // ------------------------------------------------------------------
    // Backward-compatible convenience constructors (no auto-release)
    // ------------------------------------------------------------------

    /// Create an immutable reference without tracker association.
    ///
    /// For backward compatibility. Prefer `borrow_with_tracker` for
    /// automatic borrow release on Drop.
    #[deprecated(note = "Use MemoryTracker::borrow_immutable + borrow_with_tracker instead")]
    pub fn borrow(memory_id: MemoryId) -> Self {
        Self {
            memory_id,
            borrow_type: BorrowType::Immutable,
            borrow_id: None,
            tracker_inner: None,
        }
    }

    /// Create a mutable reference without tracker association.
    #[deprecated(note = "Use MemoryTracker::borrow_mutable + borrow_mut_with_tracker instead")]
    pub fn borrow_mut(memory_id: MemoryId) -> Self {
        Self {
            memory_id,
            borrow_type: BorrowType::Mutable,
            borrow_id: None,
            tracker_inner: None,
        }
    }
}

impl Drop for SafeValue {
    fn drop(&mut self) {
        // Only release borrows that are tracked (Immutable or Mutable
        // references with an associated tracker). Owned values (Unique
        // with no borrow_id) should be deallocated explicitly.
        if let Some(ref inner_rc) = self.tracker_inner {
            if let Some(bid) = self.borrow_id {
                let mut inner = inner_rc.borrow_mut();
                // Phase 1: find and remove the borrow by ID.
                let mut emptied_mid: Option<MemoryId> = None;
                for (mid, borrows) in inner.borrows.iter_mut() {
                    if let Some(idx) = borrows.iter().position(|b| b.id == bid) {
                        borrows.remove(idx);
                        if borrows.is_empty() {
                            emptied_mid = Some(*mid);
                        }
                        break;
                    }
                }
                // Phase 2: update owner borrow state if list became empty.
                if let Some(mid) = emptied_mid {
                    if let Some(owner) = inner.owners.get_mut(&mid) {
                        owner.is_borrowed = false;
                    }
                }
                // If we didn't find the borrow, it may have been cleaned
                // up by a lifetime expiry — that's fine, no-op.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct MemorySafetyStats {
    pub total_locations: usize,
    pub active_borrows: usize,
    pub current_lifetime_depth: usize,
    pub ref_count_total: usize,
}

// ---------------------------------------------------------------------------
// Macro for safe memory operations (backward-compatible)
// ---------------------------------------------------------------------------

/// Macro for safe memory operations. Converts `MemorySafetyError` into a
/// `RuntimeError` with context. This replaces the previous version that
/// called the non-existent `RuntimeError::new`.
#[macro_export]
macro_rules! safe_memory_op {
    ($tracker:expr, $op:expr) => {{
        let result = $op;
        match result {
            Ok(value) => value,
            Err(e) => {
                return Err(pblang_ir::RuntimeError {
                    message: format!("Memory safety violation: {}", e),
                    span: None,
                    source_name: None,
                    source: None,
                    stack: vec![],
                });
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_rejects_active_borrow() {
        let mut tracker = MemoryTracker::new();
        let id = tracker.allocate(Value::Number(1.0), true);
        let _bid = tracker.borrow_immutable(id, None).unwrap();
        let err = tracker.set(id, Value::Number(2.0)).unwrap_err();
        assert!(matches!(err, MemorySafetyError::AlreadyBorrowed(_)));
    }

    #[test]
    fn unique_borrow_blocks_load() {
        let mut tracker = MemoryTracker::new();
        let id = tracker.allocate(Value::Number(1.0), true);
        let _bid = tracker.borrow_unique(id, None).unwrap();
        let err = tracker.get(id).unwrap_err();
        assert!(matches!(err, MemorySafetyError::UseAfterMove(_)));
    }
}
