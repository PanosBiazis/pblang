# loop_asm.S — hand-written x86-64 (GNU as, AT&T). Tight integer loop benchmark:
#   h = 0;  for i in [0, N):  h = (h*31 + i) & 0xFFFF;   print h
# Self-contained (itoa + write/exit syscalls, no libc). The loop token NVAL is sed-substituted by the runner.
.text
.global _start
_start:
    xor     %rbx, %rbx           # h = 0
    xor     %rcx, %rcx           # i = 0
    mov     $NVAL, %r8           # N
.Lloop:
    cmp     %r8, %rcx
    jge     .Ldone
    imul    $31, %rbx, %rax      # rax = h * 31
    add     %rcx, %rax           # + i
    and     $0xFFFF, %rax        # & 65535
    mov     %rax, %rbx           # h = rax
    inc     %rcx                 # i++
    jmp     .Lloop
.Ldone:
    sub     $32, %rsp            # itoa(h) → decimal + newline
    mov     %rbx, %rax
    mov     $10, %rcx
    lea     31(%rsp), %rsi
    movb    $10, (%rsi)
.Lconv:
    dec     %rsi
    xor     %rdx, %rdx
    div     %rcx
    add     $48, %dl
    mov     %dl, (%rsi)
    test    %rax, %rax
    jnz     .Lconv
    lea     32(%rsp), %rdx
    sub     %rsi, %rdx
    mov     $1, %rax
    mov     $1, %rdi
    syscall
    mov     $60, %rax
    xor     %rdi, %rdi
    syscall
