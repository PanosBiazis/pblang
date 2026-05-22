# tutorial_memory_safety — Memory Safety Demo

Demonstrates pblang's runtime memory safety features: ownership tracking,
borrowing rules, lifetime scopes, and error detection for use-after-free.

## Features Demonstrated

| Feature              | How it's used                                   |
|----------------------|--------------------------------------------------|
| **ownership**        | `mem.box()` allocates heap values               |
| **immutable borrow** | `mem.borrow()` for read-only shared access       |
| **mutable borrow**   | `mem.borrow_mut()` for exclusive write access    |
| **move semantics**   | `mem.free()` drops value; subsequent read errors |
| **lifetime scopes**  | `mem.scope(fn)` auto-frees values on exit        |
| **error handling**   | `try/catch` around use-after-free                |
| **memory stats**     | `mem.stats()` for runtime diagnostics            |

## Setup & Run

```bash
# From the pblang project root:
pblang run examples/tutorial_memory_safety/main.sapp
```

## Key Concepts

### 1. Ownership
Each heap-allocated value has exactly one owning handle.
Writing through the handle is safe because you are the sole owner.

### 2. Borrowing Rules (enforced at runtime)
- Multiple immutable borrows (`&T`) can coexist
- At most one mutable borrow (`&mut T`) at a time
- No borrows can coexist with a mutable borrow

### 3. Move & Free
After `mem.free(handle)`, any attempt to read the handle produces
a runtime error. Use `try/catch` for graceful handling.

### 4. Lifetime Scopes
`mem.scope(fn)` creates a new lifetime scope. All allocations
within the scope are automatically freed when it exits.

## Expected Output

```
=== 1. Ownership: Allocating & Reading ===
Allocated handle_a = 1 with value 42
Allocated handle_b = 2 with value 'hello'
Updated handle_a to 99

=== 2. Immutable Borrowing (&T) ===
Created immutable borrow 3 for handle_a
Read through handle: 99
Created second immutable borrow 4 — multiple &T borrows are OK!

=== 4. Move Semantics ===
Allocated handle_d with value 'important data'
Freed handle_d
Read after free returned: ERROR: use-after-free detected!

=== 5. Lifetime Scopes ===
  Inside scope: local value is 'scoped value'
Access after scope: correctly invalidated after scope
```
