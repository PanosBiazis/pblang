# Explicit super lookup in the class library

The PBLang-written class library supports optional explicit lookup owners:

- `super_method(instance, name, owner = nil)` returns the raw parent method.
- `super_call(instance, name, owner = nil)` invokes it with self only.
- `super1(instance, name, arg, owner = nil)` adds one argument.
- `super2(instance, name, a, b, owner = nil)` adds two arguments.

Use the class that **declares the currently executing override** as owner,
not the object's most-derived class. Its parent method table is selected;
the original instance is passed unchanged to that method.

```sapp
let C = import("stdlib:class.sapp");
let Base = C.define("Base", [], map{
    value: fn(self, x) { return x * 2; }
});
// Predeclare before the method refers to Middle: required by the checked frontend.
let Middle = nil;
Middle = C.extend(Base, "Middle", [], map{
    value: fn(self, x) { return C.super1(self, "value", x, Middle) + 1; }
});
let Leaf = C.extend(Middle, "Leaf");
assert(C.call1(C.new(Leaf), "value", 4) == 9);
```

The declaring-class binding must remain bound to that definition. Reassigning
it later changes what the closure supplies as owner; this is not a compiler-
provided immutable lexical-super binding. Methods copied by inheritance or
mixins retain the explicitly referenced owner. Mixins do not create a new
parent link or automatic method-resolution order.

## Compatibility and boundaries

Omitting owner (or passing nil) retains the old parent-of-dynamic-class lookup.
That legacy shortcut can re-enter an inherited override on a deeper subclass;
existing inheritable overrides need migration to the explicit owner form.
This addition does **not** automatically fix arbitrary old code or native
class-syntax lowering. The library cannot infer the caller's lexical owner.

Non-nil owners must pass the library's class-definition marker check. Missing
instance metadata, parent or method rejects. This is an explicit method-table
selector, not proof of ancestor membership, nominal type identity, provenance
or an authorization boundary: class/instance definitions are mutable maps.
An unrelated valid owner is not rejected on ancestry grounds. Callers must pass
the actual declaring definition; stricter identity validation remains separate.

Lookup does not mutate the instance or hold a global dispatch cursor. A parent
method sees the original dynamic class and may dispatch another override;
callback exceptions propagate without a temporary class context to restore.
Prior user callback side effects are not rolled back. No concurrency, immutable
snapshot, arbitrary-arity, multiple-inheritance or every-backend claim follows.

## Evidence

`class_super_owner` tests four-level dispatch, exact method order, zero/one/two
argument forwarding, raw lookup, bound/functional composition, mixins, parent
redispatch, errors and legacy nil/omitted-owner compatibility.
`selfhost_super_owner` imports the actual class library through the bounded
PBLang frontend/VM (20,000 steps, 32 frames, eight modules, 32-KiB source limit).
The pre-change new-API regression failed with an arity mismatch. An initial
self-hosted fixture exposed the existing self-referential initializer name
restriction; the portable example above explicitly predeclares the binding.
No Rust or compiler/VM source was changed.
