; =============================================================================
; pblang_dos16_vm.asm — Sovereign 16-bit Real-Mode x86 (8086/286) MS-DOS/FreeDOS
; Bytecode VM & Standalone Runner (`PBLANG16.COM`, ORG 0x0100) — v2.7.0
;
; Assemble:
;   nasm -f bin runtime_stage3/pblang_dos16_vm.asm -o pblang16.com
;
; Usage under MS-DOS 2.0+ / FreeDOS / DOSBox / PCem (8086, 64 KB segment):
;   C:\> PBLANG16.COM              -> Runs built-in 16-bit VM self-test & banner
;   C:\> PBLANG16.COM PROG.PB2     -> Loads & executes 16-bit stack bytecode file
;
; 16-Bit Compact Stack Bytecode (`PBC2` magic):
;   0x01 PUSH16 <imm16_le>   Push signed 16-bit integer
;   0x02 ADD16               a + b
;   0x03 SUB16               a - b
;   0x04 MUL16               a * b (signed imul)
;   0x05 DIV16               a / b (signed idiv)
;   0x06 MOD16               a % b (signed idiv remainder)
;   0x07 LOAD <slot_u8>      Push local/global slot[0..31]
;   0x08 STORE <slot_u8>     Pop into slot[0..31]
;   0x09 JZ <off_i16_le>     Pop; if zero, IP += off_i16
;   0x0A JMP <off_i16_le>    IP += off_i16
;   0x0B CMP_LT              Push (a < b ? 1 : 0)
;   0x0C CMP_EQ              Push (a == b ? 1 : 0)
;   0x0D PRINT_I16           Pop & print signed 16-bit decimal + CRLF via INT 21h
;   0x0E PRINT_STR <len_u8>  Print inline ASCII bytes via INT 21h AH=02h
;   0xFF HALT                Exit cleanly via INT 21h AH=4Ch (AL = top or 0)
; =============================================================================

BITS 16
CPU 8086
ORG 0x0100

start:
    cld
    ; Parse PSP command tail at [0x0080]: length byte, followed by ASCII args
    mov si, 0x0081
    xor cx, cx
    mov cl, [0x0080]
.skip_ws:
    cmp cx, 0
    je .run_builtin_demo
    lodsb
    dec cx
    cmp al, ' '
    je .skip_ws
    cmp al, 9
    je .skip_ws

    ; Copy filename into `fname_buf` (ASCIIZ)
    mov di, fname_buf
    stosb
.copy_fn:
    cmp cx, 0
    je .open_file
    lodsb
    dec cx
    cmp al, ' '
    je .open_file
    cmp al, 13
    je .open_file
    cmp al, 9
    je .open_file
    stosb
    jmp .copy_fn

.open_file:
    xor al, al
    stosb
    mov ax, 0x3D00          ; DOS INT 21h AH=3Dh Open existing file (read-only)
    mov dx, fname_buf
    int 0x21
    jc .file_err
    mov bx, ax              ; BX = file handle
    mov ah, 0x3F            ; DOS INT 21h AH=3Fh Read file
    mov cx, 16384
    mov dx, prog_buf
    int 0x21
    jc .file_err
    mov bp, ax              ; BP = bytes read
    mov ah, 0x3E            ; DOS INT 21h AH=3Eh Close file
    int 0x21
    cmp bp, 5
    jb .bad_magic
    mov si, prog_buf
    cmp word [si], 0x4250   ; 'PB'
    jne .bad_magic
    cmp word [si+2], 0x3243 ; 'C2'
    jne .bad_magic
    add si, 4               ; SI = bytecode IP
    jmp vm_entry

.file_err:
    mov dx, msg_file_err
    mov ah, 0x09
    int 0x21
    mov ax, 0x4C02
    int 0x21

.bad_magic:
    mov dx, msg_magic_err
    mov ah, 0x09
    int 0x21
    mov ax, 0x4C03
    int 0x21

.run_builtin_demo:
    mov dx, msg_banner
    mov ah, 0x09
    int 0x21
    mov si, demo_bytecode + 4

; ---- 16-Bit Stack VM Engine (SI = Bytecode IP, DI = Data Stack Ptr) ----
vm_entry:
    mov di, vm_stack

vm_loop:
    lodsb                   ; AL = opcode
    cmp al, 0x01            ; PUSH16
    je .op_push16
    cmp al, 0x02            ; ADD16
    je .op_add16
    cmp al, 0x03            ; SUB16
    je .op_sub16
    cmp al, 0x04            ; MUL16
    je .op_mul16
    cmp al, 0x05            ; DIV16
    je .op_div16
    cmp al, 0x06            ; MOD16
    je .op_mod16
    cmp al, 0x07            ; LOAD
    je .op_load
    cmp al, 0x08            ; STORE
    je .op_store
    cmp al, 0x09            ; JZ
    je .op_jz
    cmp al, 0x0A            ; JMP
    je .op_jmp
    cmp al, 0x0B            ; CMP_LT
    je .op_lt
    cmp al, 0x0C            ; CMP_EQ
    je .op_eq
    cmp al, 0x0D            ; PRINT_I16
    je .op_print_i16
    cmp al, 0x0E            ; PRINT_STR
    je .op_print_str
    cmp al, 0xFF            ; HALT
    je .op_halt
    mov ax, 0x4C04          ; Unknown opcode -> exit 4
    int 0x21

.op_push16:
    lodsw
    mov [di], ax
    add di, 2
    jmp vm_loop

.op_add16:
    sub di, 2
    mov bx, [di]
    sub di, 2
    add [di], bx
    add di, 2
    jmp vm_loop

.op_sub16:
    sub di, 2
    mov bx, [di]
    sub di, 2
    sub [di], bx
    add di, 2
    jmp vm_loop

.op_mul16:
    sub di, 2
    mov bx, [di]
    sub di, 2
    mov ax, [di]
    imul bx
    mov [di], ax
    add di, 2
    jmp vm_loop

.op_div16:
    sub di, 2
    mov bx, [di]
    sub di, 2
    mov ax, [di]
    cwd
    idiv bx
    mov [di], ax
    add di, 2
    jmp vm_loop

.op_mod16:
    sub di, 2
    mov bx, [di]
    sub di, 2
    mov ax, [di]
    cwd
    idiv bx
    mov [di], dx
    add di, 2
    jmp vm_loop

.op_load:
    lodsb
    xor bh, bh
    mov bl, al
    shl bx, 1
    mov ax, [vm_slots + bx]
    mov [di], ax
    add di, 2
    jmp vm_loop

.op_store:
    lodsb
    xor bh, bh
    mov bl, al
    shl bx, 1
    sub di, 2
    mov ax, [di]
    mov [vm_slots + bx], ax
    jmp vm_loop

.op_jz:
    lodsw                   ; AX = relative signed offset
    sub di, 2
    mov bx, [di]
    test bx, bx
    jnz vm_loop
    add si, ax
    jmp vm_loop

.op_jmp:
    lodsw
    add si, ax
    jmp vm_loop

.op_lt:
    sub di, 2
    mov bx, [di]
    sub di, 2
    mov ax, [di]
    xor cx, cx
    cmp ax, bx
    jge .store_cmp
    inc cx
.store_cmp:
    mov [di], cx
    add di, 2
    jmp vm_loop

.op_eq:
    sub di, 2
    mov bx, [di]
    sub di, 2
    mov ax, [di]
    xor cx, cx
    cmp ax, bx
    jne .store_cmp
    inc cx
    jmp .store_cmp

.op_print_str:
    lodsb
    xor ch, ch
    mov cl, al
    test cx, cx
    jz vm_loop
.ps_loop:
    lodsb
    mov dl, al
    mov ah, 0x02
    int 0x21
    loop .ps_loop
    jmp vm_loop

.op_print_i16:
    sub di, 2
    mov ax, [di]
    call print_ax_crlf
    jmp vm_loop

.op_halt:
    xor al, al
    cmp di, vm_stack
    jbe .exit_dos
    sub di, 2
    mov al, [di]
.exit_dos:
    mov ah, 0x4C
    int 0x21

; ---- Print signed 16-bit integer in AX + CR/LF via DOS INT 21h AH=02h ----
print_ax_crlf:
    push bx
    push cx
    push dx
    test ax, ax
    jns .pos
    push ax
    mov dl, '-'
    mov ah, 0x02
    int 0x21
    pop ax
    neg ax
.pos:
    xor cx, cx
    mov bx, 10
.div_loop:
    xor dx, dx
    div bx
    push dx
    inc cx
    test ax, ax
    jnz .div_loop
.out_loop:
    pop dx
    add dl, '0'
    mov ah, 0x02
    int 0x21
    loop .out_loop
    mov dl, 13
    mov ah, 0x02
    int 0x21
    mov dl, 10
    mov ah, 0x02
    int 0x21
    pop dx
    pop cx
    pop bx
    ret

msg_banner:
    db "pblang 2.7.0 (16-bit Real-Mode 8086 DOS Sovereign Micro-VM)", 13, 10, "$"
msg_file_err:
    db "pblang16: cannot open bytecode file", 13, 10, "$"
msg_magic_err:
    db "pblang16: invalid PBC2 header", 13, 10, "$"

; Built-in self-test bytecode: computes fib(12) = 144 iteratively and prints 144
demo_bytecode:
    db "PBC2"
    db 0x01, 0, 0           ; PUSH 0 -> slot 0 (a)
    db 0x08, 0
    db 0x01, 1, 0           ; PUSH 1 -> slot 1 (b)
    db 0x08, 1
    db 0x01, 0, 0           ; PUSH 0 -> slot 2 (i)
    db 0x08, 2
.loop_top:
    db 0x07, 2              ; LOAD i
    db 0x01, 12, 0          ; PUSH 12
    db 0x0B                 ; i < 12
    db 0x09                 ; JZ .done (+22 bytes)
    dw 22
    db 0x07, 0              ; a + b -> slot 3 (next)
    db 0x07, 1
    db 0x02
    db 0x08, 3
    db 0x07, 1              ; a = b
    db 0x08, 0
    db 0x07, 3              ; b = next
    db 0x08, 1
    db 0x07, 2              ; i = i + 1
    db 0x01, 1, 0
    db 0x02
    db 0x08, 2
    db 0x0A                 ; JMP .loop_top (-32 bytes)
    dw -32
.done:
    db 0x0E, 8, "fib(12)="
    db 0x07, 0              ; LOAD a (144)
    db 0x0D                 ; PRINT_I16
    db 0x01, 0, 0           ; exit code 0
    db 0xFF                 ; HALT

fname_buf: times 80 db 0
vm_slots:  times 32 dw 0
vm_stack:  times 128 dw 0
prog_buf:
