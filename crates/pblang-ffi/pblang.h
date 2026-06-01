/**
 * pblang.h — C API for embedding pblang (pblang) in your application.
 *
 * Build the shared library with:
 *   cargo build --release --package pblang-ffi
 *   # then link with -lpblang_ffi
 *
 * Example usage:
 *   PblangVm *vm = pblang_create_vm_sandboxed(1, 0, 0, 0, 0);  // fs only
 *   pblang_set_limits(vm, 100000, 64);  // 100k instructions, 64 frames
 *   char *out = NULL;
 *   int rc = pblang_eval(vm, "print 42;", &out);
 *   if (rc == 0 && out) { printf("Output: %s\n", out); pblang_string_free(out); }
 *   pblang_free_vm(vm);
 */

#ifndef PBLANG_H
#define PBLANG_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

/** Opaque VM handle. All operations require a valid non-null PblangVm*. */
typedef struct PblangVm PblangVm;

/**
 * Create a new pblang VM with full default capabilities.
 * Returns a non-null pointer on success. Never returns NULL.
 * Free with pblang_free_vm().
 */
PblangVm* pblang_create_vm(void);

/**
 * Create a new pblang VM with explicit capability sandbox.
 *
 * @param allow_fs      1 = allow filesystem access, 0 = deny
 * @param allow_net     1 = allow TCP/HTTP networking, 0 = deny
 * @param allow_exec    1 = allow process execution (exec, sleep_ms), 0 = deny
 * @param allow_ffi     1 = allow FFI library loading, 0 = deny
 * @param allow_sqlite  1 = allow SQLite database access, 0 = deny
 * @return Non-null VM handle. Free with pblang_free_vm().
 */
PblangVm* pblang_create_vm_sandboxed(
    int allow_fs,
    int allow_net,
    int allow_exec,
    int allow_ffi,
    int allow_sqlite
);

/**
 * Set resource limits on a VM to prevent runaway scripts.
 *
 * @param vm                Valid VM handle created by pblang_create_vm*().
 * @param max_instructions  Maximum bytecode instructions to execute (0 = unlimited).
 * @param max_stack_depth   Maximum call stack depth in frames (0 = unlimited).
 * @return 0 on success, 1 if vm is NULL.
 */
int pblang_set_limits(
    PblangVm* vm,
    uint64_t  max_instructions,
    size_t    max_stack_depth
);

/**
 * Get the pblang runtime version string.
 * @return Static NUL-terminated UTF-8 string like "0.5.1". Never NULL.
 */
const char* pblang_get_version(void);

/**
 * Get the last error message from a VM.
 * The returned pointer is owned by the VM and remains valid until the next FFI
 * call on the same VM.
 *
 * @param vm  Valid VM handle.
 * @return NUL-terminated UTF-8 error string. Empty string if no error. NULL if vm is NULL.
 */
const char* pblang_last_error(PblangVm* vm);

/**
 * Evaluate pblang source code.
 *
 * @param vm     Valid VM handle.
 * @param source NUL-terminated UTF-8 pblang source string.
 * @param out    If non-NULL, receives a pointer to a newly-allocated C string containing
 *               all printed values joined by newlines. Caller must free with pblang_string_free().
 *               Set to NULL if you don't need the output.
 * @return 0 on success, 1 on runtime/compile error, 2 if vm or source is NULL.
 *         On error, pblang_last_error(vm) contains the error message.
 */
int pblang_eval(PblangVm* vm, const char* source, char** out);

/**
 * Call a named function defined in previously evaluated source.
 *
 * @param vm        Valid VM handle (must have previously evaluated source defining fn_name).
 * @param fn_name   NUL-terminated function name.
 * @param args_json NUL-terminated JSON array of arguments, e.g. "[42, \"hello\"]".
 *                  Pass NULL or "[]" for no arguments.
 * @param out       If non-NULL, receives a newly-allocated C string with the JSON-encoded
 *                  return value. Caller must free with pblang_string_free().
 * @return 0 on success, non-zero on error.
 */
int pblang_call_function(
    PblangVm*   vm,
    const char* fn_name,
    const char* args_json,
    char**      out
);

/**
 * Free a string allocated by the pblang FFI (returned via out parameters).
 * Passing NULL is safe (no-op).
 *
 * @param s  String pointer returned by pblang_eval() or pblang_call_function() via out param.
 */
void pblang_string_free(char* s);

/**
 * Free a VM created by pblang_create_vm() or pblang_create_vm_sandboxed().
 * After this call, the vm pointer is invalid.
 * Passing NULL is safe (no-op).
 *
 * @param vm  VM handle to destroy.
 */
void pblang_free_vm(PblangVm* vm);

#ifdef __cplusplus
}  /* extern "C" */
#endif

#endif  /* PBLANG_H */
