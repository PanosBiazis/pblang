# fib_asm.S — hand-written x86-64 assembly (GNU as, AT&T syntax). Naive recursive Fibonacci with an
# itoa + write(2)/exit(2), fully self-contained (no libc). Assemble + link:
#     as --64 fib_asm.S -o fib_asm.o && ld fib_asm.o -o fib_asm
# The fib argument is the token NVAL, sed-substituted to the number by the shootout before assembling.
.text
.global _start

# fib(n): argument in %rdi, result in %rax. %rbx holds n, %rbp holds fib(n-1) across the second call.
fib:
    cmp     $2, %rdi
    jae     .Lrec
    mov     %rdi, %rax          # n < 2  ->  return n
    ret
.Lrec:
    push    %rbx
    push    %rbp
    mov     %rdi, %rbx          # rbx = n
    lea     -1(%rbx), %rdi      # fib(n-1)
    call    fib
    mov     %rax, %rbp          # save fib(n-1)
    lea     -2(%rbx), %rdi      # fib(n-2)
    call    fib
    add     %rbp, %rax          # fib(n-1) + fib(n-2)
    pop     %rbp
    pop     %rbx
    ret

_start:
    sub     $32, %rsp           # scratch buffer for the decimal string
    mov     $NVAL, %rdi         # fib(N)
    call    fib
    # itoa(%rax): digits grow down from %rsp+31, with a trailing newline at %rsp+31
    mov     $10, %rcx
    lea     31(%rsp), %rsi
    movb    $10, (%rsi)
.Lconv:
    dec     %rsi
    xor     %rdx, %rdx
    div     %rcx                # rax /= 10 ; rdx = remainder
    add     $48, %dl            # + '0'
    mov     %dl, (%rsi)
    test    %rax, %rax
    jnz     .Lconv
    lea     32(%rsp), %rdx
    sub     %rsi, %rdx          # rdx = length (digits + newline)
    mov     $1, %rax            # sys_write
    mov     $1, %rdi            # stdout
    syscall                     # rsi already points at the string
    mov     $60, %rax           # sys_exit
    xor     %rdi, %rdi
    syscall
