/*
 * zr_common.h — Sovereign Zero-Rust Runtime v3.0.0 Common OS, Crypto, UTF-8 & BigNum Kernel
 *
 * Strictly portable C99 (POSIX Linux/macOS + Windows Win32/MinGW-w64).
 * Public-safe: zero personal paths, zero host-specific identifiers, zero external deps.
 */
#ifndef PBLANG_ZR_COMMON_H
#define PBLANG_ZR_COMMON_H

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#endif
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <winhttp.h>
#include <bcrypt.h>
#include <process.h>
#include <direct.h>
#include <io.h>
#include <sys/types.h>
#include <sys/stat.h>
typedef SOCKET zr_sock_t;
#define ZR_INVALID_SOCK INVALID_SOCKET
#define zr_closesocket(s) closesocket(s)
#elif defined(__wasi__)
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#include <unistd.h>
#include <dirent.h>
#include <fcntl.h>
#include <poll.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/time.h>
typedef int zr_sock_t;
#define ZR_INVALID_SOCK (-1)
#define zr_closesocket(s) close(s)
#else
#ifdef __APPLE__
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE
#endif
#endif
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#include <unistd.h>
#include <dlfcn.h>
#include <pthread.h>
#include <dirent.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <sys/time.h>
#include <sys/utsname.h>
#include <netdb.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <arpa/inet.h>
typedef int zr_sock_t;
#define ZR_INVALID_SOCK (-1)
#define zr_closesocket(s) close(s)
#endif

#ifndef _WIN32
#include <strings.h>
#endif
#include <ctype.h>
#include <errno.h>
#include <float.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define PBLANG_ZR_VERSION "3.0.0"
#define PBLANG_ZR_BANNER  "pblang 3.0.0 (sovereign zero-rust runtime)"

/* ---- Memory Helpers ---- */

static inline void *zr_xmalloc(size_t n) {
    void *p = malloc(n ? n : 1);
    if (!p) {
        fputs("fatal: out of memory\n", stderr);
        abort();
    }
    return p;
}

static inline void *zr_xcalloc(size_t count, size_t sz) {
    void *p = calloc(count ? count : 1, sz ? sz : 1);
    if (!p) {
        fputs("fatal: out of memory\n", stderr);
        abort();
    }
    return p;
}

static inline void *zr_xrealloc(void *ptr, size_t n) {
    void *p = realloc(ptr, n ? n : 1);
    if (!p) {
        fputs("fatal: out of memory\n", stderr);
        abort();
    }
    return p;
}

static inline char *zr_xstrdup(const char *s) {
    if (!s) s = "";
    size_t n = strlen(s);
    char *out = (char *)zr_xmalloc(n + 1);
    memcpy(out, s, n + 1);
    return out;
}

static inline char *zr_xstrndup(const char *s, size_t n) {
    char *out = (char *)zr_xmalloc(n + 1);
    if (n > 0 && s) memcpy(out, s, n);
    out[n] = '\0';
    return out;
}

/* ---- Dynamic Byte / String Buffer ---- */

typedef struct {
    char  *data;
    size_t len;
    size_t cap;
} ZrBuf;

static inline void zr_buf_init(ZrBuf *b) {
    b->data = NULL;
    b->len = 0;
    b->cap = 0;
}

static inline void zr_buf_reserve(ZrBuf *b, size_t extra) {
    if (extra > SIZE_MAX / 2 || b->len > SIZE_MAX / 2) {
        fprintf(stderr, "pblang-zr: buffer overflow\n");
        abort();
    }
    size_t need = b->len + extra + 1;
    if (need <= b->cap && b->data != NULL) return;
    size_t ncap = b->cap ? b->cap * 2 : 64;
    while (ncap < need) ncap *= 2;
    b->data = (char *)zr_xrealloc(b->data, ncap);
    b->cap = ncap;
}

static inline void zr_buf_push(ZrBuf *b, char c) {
    zr_buf_reserve(b, 1);
    b->data[b->len++] = c;
    b->data[b->len] = '\0';
}

static inline void zr_buf_push_u8(ZrBuf *b, uint8_t c) {
    zr_buf_push(b, (char)c);
}

static inline void zr_buf_append(ZrBuf *b, const void *src, size_t n) {
    if (n == 0) {
        if (!b->data) {
            zr_buf_reserve(b, 0);
            b->data[0] = '\0';
        }
        return;
    }
    zr_buf_reserve(b, n);
    memcpy(b->data + b->len, src, n);
    b->len += n;
    b->data[b->len] = '\0';
}

static inline void zr_buf_append_cstr(ZrBuf *b, const char *s) {
    if (!s) return;
    zr_buf_append(b, s, strlen(s));
}

static void zr_buf_printf(ZrBuf *b, const char *fmt, ...) {
    va_list ap, ap2;
    va_start(ap, fmt);
    va_copy(ap2, ap);
    char stack_tmp[256];
    int n = vsnprintf(stack_tmp, sizeof(stack_tmp), fmt, ap);
    va_end(ap);
    if (n < 0) {
        va_end(ap2);
        return;
    }
    if ((size_t)n < sizeof(stack_tmp)) {
        zr_buf_append(b, stack_tmp, (size_t)n);
        va_end(ap2);
        return;
    }
    zr_buf_reserve(b, (size_t)n);
    vsnprintf(b->data + b->len, (size_t)n + 1, fmt, ap2);
    b->len += (size_t)n;
    va_end(ap2);
}

static inline void zr_buf_free(ZrBuf *b) {
    free(b->data);
    b->data = NULL;
    b->len = 0;
    b->cap = 0;
}

static inline char *zr_buf_take(ZrBuf *b, size_t *out_len) {
    if (!b->data) {
        b->data = zr_xstrdup("");
        b->len = 0;
        b->cap = 1;
    }
    if (out_len) *out_len = b->len;
    char *res = b->data;
    b->data = NULL;
    b->len = 0;
    b->cap = 0;
    return res;
}

/* ---- Cross-Platform Threads, Mutexes, Condvars & Dynamic Loader ---- */

#ifdef _WIN32
typedef struct { CRITICAL_SECTION cs; bool init; } ZrMutex;
typedef struct { CONDITION_VARIABLE cv; } ZrCond;
typedef HANDLE ZrThread;

static inline void zr_mutex_init(ZrMutex *m) { InitializeCriticalSection(&m->cs); m->init = true; }
static inline void zr_mutex_destroy(ZrMutex *m) { if (m->init) { DeleteCriticalSection(&m->cs); m->init = false; } }
static inline void zr_mutex_lock(ZrMutex *m) { EnterCriticalSection(&m->cs); }
static inline bool zr_mutex_trylock(ZrMutex *m) { return TryEnterCriticalSection(&m->cs) != 0; }
static inline void zr_mutex_unlock(ZrMutex *m) { LeaveCriticalSection(&m->cs); }

static inline void zr_cond_init(ZrCond *c) { InitializeConditionVariable(&c->cv); }
static inline void zr_cond_destroy(ZrCond *c) { (void)c; }
static inline void zr_cond_signal(ZrCond *c) { WakeConditionVariable(&c->cv); }
static inline void zr_cond_broadcast(ZrCond *c) { WakeAllConditionVariable(&c->cv); }
static inline void zr_cond_wait(ZrCond *c, ZrMutex *m) { SleepConditionVariableCS(&c->cv, &m->cs, INFINITE); }
static inline bool zr_cond_timedwait_ms(ZrCond *c, ZrMutex *m, uint64_t ms) {
    return SleepConditionVariableCS(&c->cv, &m->cs, (DWORD)ms) != 0;
}

typedef struct {
    void *(*fn)(void *);
    void *arg;
} ZrWinThreadStart;

static unsigned __stdcall zr_win_thread_trampoline(void *raw) {
    ZrWinThreadStart *st = (ZrWinThreadStart *)raw;
    void *(*fn)(void *) = st->fn;
    void *arg = st->arg;
    free(st);
    fn(arg);
    return 0;
}

static inline bool zr_thread_create(ZrThread *t, void *(*fn)(void *), void *arg) {
    ZrWinThreadStart *st = (ZrWinThreadStart *)zr_xmalloc(sizeof(ZrWinThreadStart));
    st->fn = fn;
    st->arg = arg;
    uintptr_t h = _beginthreadex(NULL, 0, zr_win_thread_trampoline, st, 0, NULL);
    if (!h) {
        free(st);
        return false;
    }
    *t = (HANDLE)h;
    return true;
}

static inline void zr_thread_join(ZrThread t) {
    if (t) {
        WaitForSingleObject(t, INFINITE);
        CloseHandle(t);
    }
}

static inline void *zr_dlopen(const char *path) { return (void *)LoadLibraryA(path); }
static inline void *zr_dlsym(void *handle, const char *sym) { return (void *)GetProcAddress((HMODULE)handle, sym); }
static inline void zr_dlclose(void *handle) { if (handle) FreeLibrary((HMODULE)handle); }
static inline const char *zr_dlerror(void) { return "LoadLibrary/GetProcAddress error"; }

#elif defined(__wasi__)
typedef struct { bool init; } ZrMutex;
typedef struct { bool init; } ZrCond;
typedef int ZrThread;

static inline void zr_mutex_init(ZrMutex *m) { m->init = true; }
static inline void zr_mutex_destroy(ZrMutex *m) { m->init = false; }
static inline void zr_mutex_lock(ZrMutex *m) { (void)m; }
static inline bool zr_mutex_trylock(ZrMutex *m) { (void)m; return true; }
static inline void zr_mutex_unlock(ZrMutex *m) { (void)m; }

static inline void zr_cond_init(ZrCond *c) { c->init = true; }
static inline void zr_cond_destroy(ZrCond *c) { c->init = false; }
static inline void zr_cond_signal(ZrCond *c) { (void)c; }
static inline void zr_cond_broadcast(ZrCond *c) { (void)c; }
static inline void zr_cond_wait(ZrCond *c, ZrMutex *m) { (void)c; (void)m; }
static inline bool zr_cond_timedwait_ms(ZrCond *c, ZrMutex *m, uint64_t ms) { (void)c; (void)m; (void)ms; return false; }

static inline bool zr_thread_create(ZrThread *t, void *(*fn)(void *), void *arg) { (void)t; (void)fn; (void)arg; return false; }
static inline void zr_thread_join(ZrThread t) { (void)t; }

static inline void *zr_dlopen(const char *path) { (void)path; return NULL; }
static inline void *zr_dlsym(void *handle, const char *sym) { (void)handle; (void)sym; return NULL; }
static inline void zr_dlclose(void *handle) { (void)handle; }
static inline const char *zr_dlerror(void) { return "dynamic loading unavailable under WASI"; }

#else
typedef struct { pthread_mutex_t mu; bool init; } ZrMutex;
typedef struct { pthread_cond_t cv; bool init; } ZrCond;
typedef pthread_t ZrThread;

static inline void zr_mutex_init(ZrMutex *m) { pthread_mutex_init(&m->mu, NULL); m->init = true; }
static inline void zr_mutex_destroy(ZrMutex *m) { if (m->init) { pthread_mutex_destroy(&m->mu); m->init = false; } }
static inline void zr_mutex_lock(ZrMutex *m) { pthread_mutex_lock(&m->mu); }
static inline bool zr_mutex_trylock(ZrMutex *m) { return pthread_mutex_trylock(&m->mu) == 0; }
static inline void zr_mutex_unlock(ZrMutex *m) { pthread_mutex_unlock(&m->mu); }

static inline void zr_cond_init(ZrCond *c) { pthread_cond_init(&c->cv, NULL); c->init = true; }
static inline void zr_cond_destroy(ZrCond *c) { if (c->init) { pthread_cond_destroy(&c->cv); c->init = false; } }
static inline void zr_cond_signal(ZrCond *c) { pthread_cond_signal(&c->cv); }
static inline void zr_cond_broadcast(ZrCond *c) { pthread_cond_broadcast(&c->cv); }
static inline void zr_cond_wait(ZrCond *c, ZrMutex *m) { pthread_cond_wait(&c->cv, &m->mu); }
static inline bool zr_cond_timedwait_ms(ZrCond *c, ZrMutex *m, uint64_t ms) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    ts.tv_sec += (time_t)(ms / 1000);
    ts.tv_nsec += (long)((ms % 1000) * 1000000ULL);
    if (ts.tv_nsec >= 1000000000L) {
        ts.tv_sec += 1;
        ts.tv_nsec -= 1000000000L;
    }
    return pthread_cond_timedwait(&c->cv, &m->mu, &ts) == 0;
}

static inline bool zr_thread_create(ZrThread *t, void *(*fn)(void *), void *arg) {
    return pthread_create(t, NULL, fn, arg) == 0;
}

static inline void zr_thread_join(ZrThread t) {
    pthread_join(t, NULL);
}

static inline void *zr_dlopen(const char *path) { return dlopen(path, RTLD_LAZY | RTLD_LOCAL); }
static inline void *zr_dlsym(void *handle, const char *sym) { return dlsym(handle, sym); }
static inline void zr_dlclose(void *handle) { if (handle) dlclose(handle); }
static inline const char *zr_dlerror(void) { const char *e = dlerror(); return e ? e : "dlopen error"; }
#endif

/* ---- Time Helpers ---- */

static inline double zr_mono_sec(void) {
#ifdef _WIN32
    static LARGE_INTEGER freq = {0};
    if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    return (double)now.QuadPart / (double)freq.QuadPart;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
#endif
}

static inline double zr_now_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    ULARGE_INTEGER ui;
    ui.LowPart = ft.dwLowDateTime;
    ui.HighPart = ft.dwHighDateTime;
    uint64_t ticks = ui.QuadPart - 116444736000000000ULL;
    return (double)(ticks / 10000ULL);
#else
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1.0e6;
#endif
}

static inline void zr_now_iso(char out[40]) {
    time_t now_s = time(NULL);
    struct tm gmt;
#ifdef _WIN32
    gmtime_s(&gmt, &now_s);
#else
    gmtime_r(&now_s, &gmt);
#endif
    strftime(out, 40, "%Y-%m-%dT%H:%M:%SZ", &gmt);
}

static inline void zr_sleep_ms(double ms) {
    if (ms <= 0.0 || isnan(ms)) return;
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec ts;
    uint64_t ns_total = (uint64_t)(ms * 1000000.0);
    ts.tv_sec = (time_t)(ns_total / 1000000000ULL);
    ts.tv_nsec = (long)(ns_total % 1000000000ULL);
    nanosleep(&ts, NULL);
#endif
}

/* ---- Socket Bootstrap (Windows WSAStartup) ---- */

static inline void zr_net_ensure_init(void) {
#ifdef _WIN32
    static int initialized = 0;
    if (!initialized) {
        WSADATA wsa;
        WSAStartup(MAKEWORD(2, 2), &wsa);
        initialized = 1;
    }
#endif
}

/* ---- UTF-8 & Unicode Identifier Helpers ---- */

static inline uint32_t zr_utf8_decode(const char *s, size_t len, size_t *adv) {
    if (len == 0) {
        *adv = 0;
        return 0;
    }
    uint8_t b0 = (uint8_t)s[0];
    if (b0 < 0x80) {
        *adv = 1;
        return b0;
    }
    if ((b0 & 0xE0) == 0xC0 && len >= 2) {
        uint8_t b1 = (uint8_t)s[1];
        if ((b1 & 0xC0) == 0x80) {
            uint32_t cp = ((uint32_t)(b0 & 0x1F) << 6) | (uint32_t)(b1 & 0x3F);
            if (cp >= 0x80) {
                *adv = 2;
                return cp;
            }
        }
    } else if ((b0 & 0xF0) == 0xE0 && len >= 3) {
        uint8_t b1 = (uint8_t)s[1], b2 = (uint8_t)s[2];
        if ((b1 & 0xC0) == 0x80 && (b2 & 0xC0) == 0x80) {
            uint32_t cp = ((uint32_t)(b0 & 0x0F) << 12) | ((uint32_t)(b1 & 0x3F) << 6) | (uint32_t)(b2 & 0x3F);
            if (cp >= 0x800) {
                *adv = 3;
                return cp;
            }
        }
    } else if ((b0 & 0xF8) == 0xF0 && len >= 4) {
        uint8_t b1 = (uint8_t)s[1], b2 = (uint8_t)s[2], b3 = (uint8_t)s[3];
        if ((b1 & 0xC0) == 0x80 && (b2 & 0xC0) == 0x80 && (b3 & 0xC0) == 0x80) {
            uint32_t cp = ((uint32_t)(b0 & 0x07) << 18) | ((uint32_t)(b1 & 0x3F) << 12) |
                          ((uint32_t)(b2 & 0x3F) << 6) | (uint32_t)(b3 & 0x3F);
            if (cp >= 0x10000 && cp <= 0x10FFFF) {
                *adv = 4;
                return cp;
            }
        }
    }
    *adv = 1;
    return 0xFFFD;
}

static inline size_t zr_utf8_encode(uint32_t cp, void *out_raw) {
    char *out = (char *)out_raw;
    if (cp <= 0x7F) {
        out[0] = (char)cp;
        return 1;
    } else if (cp <= 0x7FF) {
        out[0] = (char)(0xC0 | (cp >> 6));
        out[1] = (char)(0x80 | (cp & 0x3F));
        return 2;
    } else if (cp <= 0xFFFF) {
        out[0] = (char)(0xE0 | (cp >> 12));
        out[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[2] = (char)(0x80 | (cp & 0x3F));
        return 3;
    } else if (cp <= 0x10FFFF) {
        out[0] = (char)(0xF0 | (cp >> 18));
        out[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
        out[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[3] = (char)(0x80 | (cp & 0x3F));
        return 4;
    }
    out[0] = '?';
    return 1;
}

static inline size_t zr_utf8_char_len(const char *s, size_t byte_len) {
    size_t i = 0, count = 0;
    while (i < byte_len) {
        size_t adv = 1;
        zr_utf8_decode(s + i, byte_len - i, &adv);
        i += adv ? adv : 1;
        count++;
    }
    return count;
}

static inline size_t zr_utf8_nth_offset(const char *s, size_t byte_len, size_t char_idx, size_t *out_adv) {
    size_t i = 0, count = 0;
    while (i < byte_len) {
        size_t adv = 1;
        zr_utf8_decode(s + i, byte_len - i, &adv);
        if (count == char_idx) {
            if (out_adv) *out_adv = adv;
            return i;
        }
        i += adv ? adv : 1;
        count++;
    }
    if (out_adv) *out_adv = 0;
    return byte_len;
}

static inline bool zr_is_unicode_non_ident(uint32_t cp) {
    /* Exclude Unicode whitespace, general punctuation, math operators, arrows, box drawing */
    if (cp <= 0x00A0) return true;
    if (cp == 0x00AD || cp == 0x00B7 || cp == 0x00D7 || cp == 0x00F7) return true;
    if (cp >= 0x2000 && cp <= 0x2BFF) return true;
    if (cp >= 0x3000 && cp <= 0x303F) return true;
    if (cp >= 0xFE00 && cp <= 0xFE6F) return true;
    if (cp >= 0xFF00 && cp <= 0xFF0F) return true;
    if (cp == 0xFEFF || cp == 0xFFFD) return true;
    return false;
}

static inline bool zr_is_ident_start_cp(uint32_t cp) {
    if (cp < 0x80) {
        return (cp == '_') || (cp >= 'a' && cp <= 'z') || (cp >= 'A' && cp <= 'Z');
    }
    return !zr_is_unicode_non_ident(cp);
}

static inline bool zr_is_ident_continue_cp(uint32_t cp) {
    if (cp < 0x80) {
        return (cp == '_') || (cp >= 'a' && cp <= 'z') || (cp >= 'A' && cp <= 'Z') || (cp >= '0' && cp <= '9');
    }
    if (cp >= 0x0300 && cp <= 0x036F) return true; /* combining marks */
    return !zr_is_unicode_non_ident(cp);
}

/* Basic Unicode case conversion for ASCII + Greek */
static inline uint32_t zr_cp_to_upper(uint32_t cp) {
    if (cp >= 'a' && cp <= 'z') return cp - 32;
    if (cp >= 0x03B1 && cp <= 0x03C1) return cp - 0x20; /* α..ρ -> Α..Ρ */
    if (cp == 0x03C2) return 0x03A3;                     /* ς -> Σ */
    if (cp >= 0x03C3 && cp <= 0x03CB) return cp - 0x20; /* σ..ϋ -> Σ..Ϋ */
    if (cp == 0x03AC) return 0x0386;                     /* ά -> Ά */
    if (cp == 0x03AD) return 0x0388;                     /* έ -> Έ */
    if (cp == 0x03AE) return 0x0389;                     /* ή -> Ή */
    if (cp == 0x03AF) return 0x038A;                     /* ί -> Ί */
    if (cp == 0x03CC) return 0x038C;                     /* ό -> Ό */
    if (cp == 0x03CD) return 0x038E;                     /* ύ -> Ύ */
    if (cp == 0x03CE) return 0x038F;                     /* ώ -> Ώ */
    return cp;
}

static inline uint32_t zr_cp_to_lower(uint32_t cp) {
    if (cp >= 'A' && cp <= 'Z') return cp + 32;
    if (cp >= 0x0391 && cp <= 0x03A1) return cp + 0x20; /* Α..Ρ -> α..ρ */
    if (cp >= 0x03A3 && cp <= 0x03AB) return cp + 0x20; /* Σ..Ϋ -> σ..ϋ */
    if (cp == 0x0386) return 0x03AC;
    if (cp == 0x0388) return 0x03AD;
    if (cp == 0x0389) return 0x03AE;
    if (cp == 0x038A) return 0x03AF;
    if (cp == 0x038C) return 0x03CC;
    if (cp == 0x038E) return 0x03CD;
    if (cp == 0x038F) return 0x03CE;
    return cp;
}

/* ---- FNV-1a 64-bit ---- */

static inline uint64_t zr_fnv1a(const void *data, size_t len) {
    const uint8_t *p = (const uint8_t *)data;
    uint64_t h = 14695981039346656037ULL;
    for (size_t i = 0; i < len; i++) {
        h ^= (uint64_t)p[i];
        h *= 1099511628211ULL;
    }
    return h ? h : 1;
}

/* ---- SHA-1 (RFC 3174 / RFC 6455 WebSocket Accept Key) ---- */

static void zr_sha1(const uint8_t *msg, size_t len, uint8_t out[20]) {
    uint32_t h0 = 0x67452301u, h1 = 0xEFCDAB89u, h2 = 0x98BADCFEu, h3 = 0x10325476u, h4 = 0xC3D2E1F0u;
    size_t new_len = len + 1;
    while ((new_len % 64) != 56) new_len++;
    uint8_t *buf = (uint8_t *)zr_xcalloc(new_len + 8, 1);
    if (len > 0) memcpy(buf, msg, len);
    buf[len] = 0x80;
    uint64_t bit_len = (uint64_t)len * 8ULL;
    for (int i = 0; i < 8; i++) buf[new_len + i] = (uint8_t)((bit_len >> (56 - 8 * i)) & 0xFFu);

    for (size_t off = 0; off < new_len + 8; off += 64) {
        uint32_t w[80];
        for (int i = 0; i < 16; i++) {
            w[i] = ((uint32_t)buf[off + i * 4] << 24) |
                   ((uint32_t)buf[off + i * 4 + 1] << 16) |
                   ((uint32_t)buf[off + i * 4 + 2] << 8) |
                   ((uint32_t)buf[off + i * 4 + 3]);
        }
        for (int i = 16; i < 80; i++) {
            uint32_t x = w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16];
            w[i] = (x << 1) | (x >> 31);
        }
        uint32_t a = h0, b = h1, c = h2, d = h3, e = h4;
        for (int i = 0; i < 80; i++) {
            uint32_t f, k;
            if (i < 20) {
                f = (b & c) | ((~b) & d);
                k = 0x5A827999u;
            } else if (i < 40) {
                f = b ^ c ^ d;
                k = 0x6ED9EBA1u;
            } else if (i < 60) {
                f = (b & c) | (b & d) | (c & d);
                k = 0x8F1BBCDCu;
            } else {
                f = b ^ c ^ d;
                k = 0xCA62C1D6u;
            }
            uint32_t temp = ((a << 5) | (a >> 27)) + f + e + k + w[i];
            e = d;
            d = c;
            c = (b << 30) | (b >> 2);
            b = a;
            a = temp;
        }
        h0 += a; h1 += b; h2 += c; h3 += d; h4 += e;
    }
    free(buf);
    uint32_t hs[5] = {h0, h1, h2, h3, h4};
    for (int i = 0; i < 5; i++) {
        out[i * 4 + 0] = (uint8_t)(hs[i] >> 24);
        out[i * 4 + 1] = (uint8_t)(hs[i] >> 16);
        out[i * 4 + 2] = (uint8_t)(hs[i] >> 8);
        out[i * 4 + 3] = (uint8_t)(hs[i]);
    }
}

/* ---- SHA-256 & HMAC-SHA256 (FIPS 180-4) ---- */

static inline uint32_t zr_rotr32(uint32_t x, uint32_t n) { return (x >> n) | (x << (32 - n)); }

static void zr_sha256(const uint8_t *msg, size_t len, uint8_t out[32]) {
    static const uint32_t K[64] = {
        0x428a2f98u,0x71374491u,0xb5c0fbcfu,0xe9b5dba5u,0x3956c25bu,0x59f111f1u,0x923f82a4u,0xab1c5ed5u,
        0xd807aa98u,0x12835b01u,0x243185beu,0x550c7dc3u,0x72be5d74u,0x80deb1feu,0x9bdc06a7u,0xc19bf174u,
        0xe49b69c1u,0xefbe4786u,0x0fc19dc6u,0x240ca1ccu,0x2de92c6fu,0x4a7484aau,0x5cb0a9dcu,0x76f988dau,
        0x983e5152u,0xa831c66du,0xb00327c8u,0xbf597fc7u,0xc6e00bf3u,0xd5a79147u,0x06ca6351u,0x14292967u,
        0x27b70a85u,0x2e1b2138u,0x4d2c6dfcu,0x53380d13u,0x650a7354u,0x766a0abbu,0x81c2c92eu,0x92722c85u,
        0xa2bfe8a1u,0xa81a664bu,0xc24b8b70u,0xc76c51a3u,0xd192e819u,0xd6990624u,0xf40e3585u,0x106aa070u,
        0x19a4c116u,0x1e376c08u,0x2748774cu,0x34b0bcb5u,0x391c0cb3u,0x4ed8aa4au,0x5b9cca4fu,0x682e6ff3u,
        0x748f82eeu,0x78a5636fu,0x84c87814u,0x8cc70208u,0x90befffau,0xa4506cebu,0xbef9a3f7u,0xc67178f2u
    };
    uint32_t h[8] = {
        0x6a09e667u,0xbb67ae85u,0x3c6ef372u,0xa54ff53au,
        0x510e527fu,0x9b05688cu,0x1f83d9abu,0x5be0cd19u
    };
    size_t new_len = len + 1;
    while ((new_len % 64) != 56) new_len++;
    uint8_t *buf = (uint8_t *)zr_xcalloc(new_len + 8, 1);
    if (len > 0) memcpy(buf, msg, len);
    buf[len] = 0x80;
    uint64_t bit_len = (uint64_t)len * 8ULL;
    for (int i = 0; i < 8; i++) buf[new_len + i] = (uint8_t)((bit_len >> (56 - 8 * i)) & 0xFFu);

    for (size_t off = 0; off < new_len + 8; off += 64) {
        uint32_t w[64];
        for (int i = 0; i < 16; i++) {
            w[i] = ((uint32_t)buf[off + i * 4] << 24) |
                   ((uint32_t)buf[off + i * 4 + 1] << 16) |
                   ((uint32_t)buf[off + i * 4 + 2] << 8) |
                   ((uint32_t)buf[off + i * 4 + 3]);
        }
        for (int i = 16; i < 64; i++) {
            uint32_t s0 = zr_rotr32(w[i - 15], 7) ^ zr_rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
            uint32_t s1 = zr_rotr32(w[i - 2], 17) ^ zr_rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
        for (int i = 0; i < 64; i++) {
            uint32_t S1 = zr_rotr32(e, 6) ^ zr_rotr32(e, 11) ^ zr_rotr32(e, 25);
            uint32_t ch = (e & f) ^ ((~e) & g);
            uint32_t t1 = hh + S1 + ch + K[i] + w[i];
            uint32_t S0 = zr_rotr32(a, 2) ^ zr_rotr32(a, 13) ^ zr_rotr32(a, 22);
            uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint32_t t2 = S0 + maj;
            hh = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d;
        h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }
    free(buf);
    for (int i = 0; i < 8; i++) {
        out[i * 4 + 0] = (uint8_t)(h[i] >> 24);
        out[i * 4 + 1] = (uint8_t)(h[i] >> 16);
        out[i * 4 + 2] = (uint8_t)(h[i] >> 8);
        out[i * 4 + 3] = (uint8_t)(h[i]);
    }
}

static void zr_hmac_sha256(const uint8_t *key, size_t klen, const uint8_t *msg, size_t mlen, uint8_t out[32]) {
    uint8_t kblock[64];
    memset(kblock, 0, sizeof(kblock));
    if (klen > 64) {
        zr_sha256(key, klen, kblock);
    } else if (klen > 0) {
        memcpy(kblock, key, klen);
    }
    uint8_t o_pad[64], i_pad[64];
    for (int i = 0; i < 64; i++) {
        o_pad[i] = kblock[i] ^ 0x5Cu;
        i_pad[i] = kblock[i] ^ 0x36u;
    }
    uint8_t *inner = (uint8_t *)zr_xmalloc(64 + mlen);
    memcpy(inner, i_pad, 64);
    if (mlen > 0) memcpy(inner + 64, msg, mlen);
    uint8_t inner_hash[32];
    zr_sha256(inner, 64 + mlen, inner_hash);
    free(inner);

    uint8_t outer[96];
    memcpy(outer, o_pad, 64);
    memcpy(outer + 64, inner_hash, 32);
    zr_sha256(outer, 96, out);
}

/* ---- SHA-512 (FIPS 180-4) ---- */

static inline uint64_t zr_rotr64(uint64_t x, uint64_t n) { return (x >> n) | (x << (64 - n)); }

static void zr_sha512(const uint8_t *msg, size_t len, uint8_t out[64]) {
    static const uint64_t K[80] = {
        0x428a2f98d728ae22ULL,0x7137449123ef65cdULL,0xb5c0fbcfec4d3b2fULL,0xe9b5dba58189dbbcULL,
        0x3956c25bf348b538ULL,0x59f111f1b605d019ULL,0x923f82a4af194f9bULL,0xab1c5ed5da6d8118ULL,
        0xd807aa98a3030242ULL,0x12835b0145706fbeULL,0x243185be4ee4b28cULL,0x550c7dc3d5ffb4e2ULL,
        0x72be5d74f27b896fULL,0x80deb1fe3b1696b1ULL,0x9bdc06a725c71235ULL,0xc19bf174cf692694ULL,
        0xe49b69c19ef14ad2ULL,0xefbe4786384f25e3ULL,0x0fc19dc68b8cd5b5ULL,0x240ca1cc77ac9c65ULL,
        0x2de92c6f592b0275ULL,0x4a7484aa6ea6e483ULL,0x5cb0a9dcbd41fbd4ULL,0x76f988da831153b5ULL,
        0x983e5152ee66dfabULL,0xa831c66d2db43210ULL,0xb00327c898fb213fULL,0xbf597fc7beef0ee4ULL,
        0xc6e00bf33da88fc2ULL,0xd5a79147930aa725ULL,0x06ca6351e003826fULL,0x142929670a0e6e70ULL,
        0x27b70a8546d22ffcULL,0x2e1b21385c26c926ULL,0x4d2c6dfc5ac42aedULL,0x53380d139d95b3dfULL,
        0x650a73548baf63deULL,0x766a0abb3c77b2a8ULL,0x81c2c92e47edaee6ULL,0x92722c851482353bULL,
        0xa2bfe8a14cf10364ULL,0xa81a664bbc423001ULL,0xc24b8b70d0f89791ULL,0xc76c51a30654be30ULL,
        0xd192e819d6ef5218ULL,0xd69906245565a910ULL,0xf40e35855771202aULL,0x106aa07032bbd1b8ULL,
        0x19a4c116b8d2d0c8ULL,0x1e376c085141ab53ULL,0x2748774cdf8eeb99ULL,0x34b0bcb5e19b48a8ULL,
        0x391c0cb3c5c95a63ULL,0x4ed8aa4ae3418acbULL,0x5b9cca4f7763e373ULL,0x682e6ff3d6b2b8a3ULL,
        0x748f82ee5defb2fcULL,0x78a5636f43172f60ULL,0x84c87814a1f0ab72ULL,0x8cc702081a6439ecULL,
        0x90befffa23631e28ULL,0xa4506cebde82bde9ULL,0xbef9a3f7b2c67915ULL,0xc67178f2e372532bULL,
        0xca273eceea26619cULL,0xd186b8c721c0c207ULL,0xeada7dd6cde0eb1eULL,0xf57d4f7fee6ed178ULL,
        0x06f067aa72176fbaULL,0x0a637dc5a2c898a6ULL,0x113f9804bef90daeULL,0x1b710b35131c471bULL,
        0x28db77f523047d84ULL,0x32caab7b40c72493ULL,0x3c9ebe0a15c9bebcULL,0x431d67c49c100d4cULL,
        0x4cc5d4becb3e42b6ULL,0x597f299cfc657e2aULL,0x5fcb6fab3ad6faecULL,0x6c44198c4a475817ULL
    };
    uint64_t h[8] = {
        0x6a09e667f3bcc908ULL,0xbb67ae8584caa73bULL,0x3c6ef372fe94f82bULL,0xa54ff53a5f1d36f1ULL,
        0x510e527fade682d1ULL,0x9b05688c2b3e6c1fULL,0x1f83d9abfb41bd6bULL,0x5be0cd19137e2179ULL
    };
    size_t new_len = len + 1;
    while ((new_len % 128) != 112) new_len++;
    uint8_t *buf = (uint8_t *)zr_xcalloc(new_len + 16, 1);
    if (len > 0) memcpy(buf, msg, len);
    buf[len] = 0x80;
    uint64_t bit_len = (uint64_t)len * 8ULL;
    for (int i = 0; i < 8; i++) buf[new_len + 8 + i] = (uint8_t)((bit_len >> (56 - 8 * i)) & 0xFFu);

    for (size_t off = 0; off < new_len + 16; off += 128) {
        uint64_t w[80];
        for (int i = 0; i < 16; i++) {
            uint64_t v = 0;
            for (int b = 0; b < 8; b++) v = (v << 8) | buf[off + i * 8 + b];
            w[i] = v;
        }
        for (int i = 16; i < 80; i++) {
            uint64_t s0 = zr_rotr64(w[i - 15], 1) ^ zr_rotr64(w[i - 15], 8) ^ (w[i - 15] >> 7);
            uint64_t s1 = zr_rotr64(w[i - 2], 19) ^ zr_rotr64(w[i - 2], 61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint64_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
        for (int i = 0; i < 80; i++) {
            uint64_t S1 = zr_rotr64(e, 14) ^ zr_rotr64(e, 18) ^ zr_rotr64(e, 41);
            uint64_t ch = (e & f) ^ ((~e) & g);
            uint64_t t1 = hh + S1 + ch + K[i] + w[i];
            uint64_t S0 = zr_rotr64(a, 28) ^ zr_rotr64(a, 34) ^ zr_rotr64(a, 39);
            uint64_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint64_t t2 = S0 + maj;
            hh = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d;
        h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }
    free(buf);
    for (int i = 0; i < 8; i++) {
        for (int b = 0; b < 8; b++) {
            out[i * 8 + b] = (uint8_t)((h[i] >> (56 - 8 * b)) & 0xFFu);
        }
    }
}

/* ---- CSPRNG ---- */

static bool zr_csprng_fill(uint8_t *buf, size_t len) {
    if (len == 0) return true;
#ifdef _WIN32
    return BCryptGenRandom(NULL, buf, (ULONG)len, BCRYPT_USE_SYSTEM_PREFERRED_RNG) == 0;
#else
    int fd = open("/dev/urandom", O_RDONLY);
    if (fd < 0) return false;
    size_t got = 0;
    while (got < len) {
        ssize_t r = read(fd, buf + got, len - got);
        if (r <= 0) {
            if (errno == EINTR) continue;
            close(fd);
            return false;
        }
        got += (size_t)r;
    }
    close(fd);
    return true;
#endif
}

/* ---- Hex / Base64 / URL Encoding ---- */

static void zr_hex_encode_raw(const uint8_t *src, size_t len, char *out) {
    static const char *HEX = "0123456789abcdef";
    for (size_t i = 0; i < len; i++) {
        out[i * 2]     = HEX[(src[i] >> 4) & 0x0Fu];
        out[i * 2 + 1] = HEX[src[i] & 0x0Fu];
    }
    out[len * 2] = '\0';
}

static inline int zr_hex_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static char *zr_b64_encode(const uint8_t *src, size_t len, size_t *out_len) {
    static const char *B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    size_t olen = ((len + 2) / 3) * 4;
    char *out = (char *)zr_xmalloc(olen + 1);
    size_t j = 0;
    for (size_t i = 0; i < len; i += 3) {
        uint32_t a = src[i];
        uint32_t b = (i + 1 < len) ? src[i + 1] : 0;
        uint32_t c = (i + 2 < len) ? src[i + 2] : 0;
        uint32_t t = (a << 16) | (b << 8) | c;
        out[j++] = B64[(t >> 18) & 63];
        out[j++] = B64[(t >> 12) & 63];
        out[j++] = (i + 1 < len) ? B64[(t >> 6) & 63] : '=';
        out[j++] = (i + 2 < len) ? B64[t & 63] : '=';
    }
    out[j] = '\0';
    if (out_len) *out_len = j;
    return out;
}

static bool zr_b64_decode(const char *src, size_t len, uint8_t **out_buf, size_t *out_len) {
    int8_t tbl[256];
    memset(tbl, -1, sizeof(tbl));
    const char *B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (int i = 0; i < 64; i++) tbl[(uint8_t)B64[i]] = (int8_t)i;
    tbl[(uint8_t)'-'] = 62;
    tbl[(uint8_t)'_'] = 63;

    uint8_t *out = (uint8_t *)zr_xmalloc((len / 4 + 1) * 3 + 1);
    size_t olen = 0;
    uint32_t val = 0;
    int valb = -8;
    for (size_t i = 0; i < len; i++) {
        uint8_t c = (uint8_t)src[i];
        if (c == '=' || c == ' ' || c == '\r' || c == '\n' || c == '\t') {
            if (c == '=') break;
            continue;
        }
        int8_t d = tbl[c];
        if (d < 0) {
            free(out);
            return false;
        }
        val = (val << 6) | (uint32_t)d;
        valb += 6;
        if (valb >= 0) {
            out[olen++] = (uint8_t)((val >> valb) & 0xFFu);
            valb -= 8;
        }
    }
    out[olen] = 0;
    *out_buf = out;
    *out_len = olen;
    return true;
}

/* ---- Arbitrary-Precision BigInt (Base 10^9 Limbs), Rational & Decimal ---- */

#define ZR_BI_BASE 1000000000U

typedef struct {
    int       sign;  /* 0 = zero, 1 = positive, -1 = negative */
    uint32_t *limbs; /* little-endian base 10^9 limbs */
    size_t    len;
    size_t    cap;
} ZrBigInt;

typedef struct {
    ZrBigInt num;
    ZrBigInt den; /* always > 0 */
} ZrRational;

typedef struct {
    ZrBigInt coeff;
    int32_t  scale; /* value = coeff * 10^(-scale), scale >= 0 */
} ZrDecimal;

static inline void zr_bi_init(ZrBigInt *a) {
    a->sign = 0;
    a->limbs = NULL;
    a->len = 0;
    a->cap = 0;
}

static inline void zr_bi_free(ZrBigInt *a) {
    free(a->limbs);
    a->limbs = NULL;
    a->len = 0;
    a->cap = 0;
    a->sign = 0;
}

static inline void zr_bi_reserve(ZrBigInt *a, size_t cap) {
    if (cap <= a->cap) return;
    size_t ncap = a->cap ? a->cap * 2 : 4;
    while (ncap < cap) ncap *= 2;
    a->limbs = (uint32_t *)zr_xrealloc(a->limbs, ncap * sizeof(uint32_t));
    a->cap = ncap;
}

static inline void zr_bi_trim(ZrBigInt *a) {
    while (a->len > 0 && a->limbs[a->len - 1] == 0) a->len--;
    if (a->len == 0) a->sign = 0;
}

static inline void zr_bi_copy(ZrBigInt *dst, const ZrBigInt *src) {
    zr_bi_init(dst);
    if (src->sign == 0 || src->len == 0) return;
    zr_bi_reserve(dst, src->len);
    memcpy(dst->limbs, src->limbs, src->len * sizeof(uint32_t));
    dst->len = src->len;
    dst->sign = src->sign;
}

static inline void zr_bi_from_i64(ZrBigInt *a, int64_t v) {
    zr_bi_init(a);
    if (v == 0) return;
    uint64_t u;
    if (v < 0) {
        a->sign = -1;
        u = (uint64_t)(-(v + 1)) + 1ULL;
    } else {
        a->sign = 1;
        u = (uint64_t)v;
    }
    zr_bi_reserve(a, 3);
    while (u > 0) {
        a->limbs[a->len++] = (uint32_t)(u % ZR_BI_BASE);
        u /= ZR_BI_BASE;
    }
}

static inline void zr_bi_mul_u32_ip(ZrBigInt *a, uint32_t m) {
    if (a->sign == 0 || m == 0) {
        a->len = 0;
        a->sign = 0;
        return;
    }
    uint64_t carry = 0;
    for (size_t i = 0; i < a->len; i++) {
        uint64_t cur = (uint64_t)a->limbs[i] * (uint64_t)m + carry;
        a->limbs[i] = (uint32_t)(cur % ZR_BI_BASE);
        carry = cur / ZR_BI_BASE;
    }
    while (carry > 0) {
        zr_bi_reserve(a, a->len + 1);
        a->limbs[a->len++] = (uint32_t)(carry % ZR_BI_BASE);
        carry /= ZR_BI_BASE;
    }
}

static inline void zr_bi_add_u32_ip(ZrBigInt *a, uint32_t v) {
    if (v == 0) return;
    if (a->sign == 0) {
        a->sign = 1;
        zr_bi_reserve(a, 1);
        a->limbs[0] = v;
        a->len = 1;
        return;
    }
    uint64_t carry = v;
    for (size_t i = 0; i < a->len && carry > 0; i++) {
        uint64_t cur = (uint64_t)a->limbs[i] + carry;
        a->limbs[i] = (uint32_t)(cur % ZR_BI_BASE);
        carry = cur / ZR_BI_BASE;
    }
    if (carry > 0) {
        zr_bi_reserve(a, a->len + 1);
        a->limbs[a->len++] = (uint32_t)carry;
    }
}

static bool zr_bi_from_str(ZrBigInt *out, const char *s) {
    zr_bi_init(out);
    if (!s) return false;
    while (*s && isspace((unsigned char)*s)) s++;
    int sign = 1;
    if (*s == '-') { sign = -1; s++; }
    else if (*s == '+') { s++; }
    if (!*s) return false;

    uint32_t radix = 10;
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) { radix = 16; s += 2; }
    else if (s[0] == '0' && (s[1] == 'b' || s[1] == 'B')) { radix = 2; s += 2; }
    else if (s[0] == '0' && (s[1] == 'o' || s[1] == 'O')) { radix = 8; s += 2; }

    size_t digits = 0;
    for (const char *p = s; *p; p++) {
        if (*p == '_') continue;
        if (isspace((unsigned char)*p)) {
            while (*p && isspace((unsigned char)*p)) p++;
            if (*p != '\0') { zr_bi_free(out); return false; }
            break;
        }
        int d = zr_hex_val(*p);
        if (d < 0 || (uint32_t)d >= radix) {
            zr_bi_free(out);
            return false;
        }
        zr_bi_mul_u32_ip(out, radix);
        zr_bi_add_u32_ip(out, (uint32_t)d);
        digits++;
    }
    if (digits == 0) {
        zr_bi_free(out);
        return false;
    }
    zr_bi_trim(out);
    if (out->sign != 0) out->sign = sign;
    return true;
}

static bool zr_bi_from_f64(ZrBigInt *out, double f) {
    zr_bi_init(out);
    if (!isfinite(f) || floor(f) != f) return false;
    if (f == 0.0) return true;
    int sign = (f < 0.0) ? -1 : 1;
    double a = fabs(f);
    if (a <= 9007199254740992.0) {
        zr_bi_from_i64(out, (int64_t)a);
        if (out->sign != 0) out->sign = sign;
        return true;
    }
    char buf[400];
    snprintf(buf, sizeof(buf), "%.0f", a);
    if (!zr_bi_from_str(out, buf)) return false;
    if (out->sign != 0) out->sign = sign;
    return true;
}

static char *zr_bi_to_str(const ZrBigInt *a) {
    if (a->sign == 0 || a->len == 0) return zr_xstrdup("0");
    ZrBuf b;
    zr_buf_init(&b);
    zr_buf_reserve(&b, a->len * 9 + 2);
    if (a->sign < 0) zr_buf_push(&b, '-');
    zr_buf_printf(&b, "%u", a->limbs[a->len - 1]);
    for (size_t i = a->len - 1; i > 0; i--) {
        zr_buf_printf(&b, "%09u", a->limbs[i - 1]);
    }
    return zr_buf_take(&b, NULL);
}

static double zr_bi_to_f64(const ZrBigInt *a) {
    if (a->sign == 0 || a->len == 0) return 0.0;
    double v = 0.0;
    for (size_t i = a->len; i > 0; i--) {
        v = v * (double)ZR_BI_BASE + (double)a->limbs[i - 1];
    }
    return a->sign < 0 ? -v : v;
}

static int zr_bi_cmp_abs(const ZrBigInt *a, const ZrBigInt *b) {
    if (a->len != b->len) return (a->len < b->len) ? -1 : 1;
    for (size_t i = a->len; i > 0; i--) {
        uint32_t la = a->limbs[i - 1], lb = b->limbs[i - 1];
        if (la != lb) return (la < lb) ? -1 : 1;
    }
    return 0;
}

static int zr_bi_cmp(const ZrBigInt *a, const ZrBigInt *b) {
    if (a->sign != b->sign) return (a->sign < b->sign) ? -1 : 1;
    if (a->sign == 0) return 0;
    int c = zr_bi_cmp_abs(a, b);
    return (a->sign > 0) ? c : -c;
}

static void zr_bi_add_abs(ZrBigInt *out, const ZrBigInt *a, const ZrBigInt *b) {
    zr_bi_init(out);
    size_t n = (a->len > b->len) ? a->len : b->len;
    if (n == 0) return;
    zr_bi_reserve(out, n + 1);
    uint64_t carry = 0;
    for (size_t i = 0; i < n; i++) {
        uint64_t va = (i < a->len) ? a->limbs[i] : 0;
        uint64_t vb = (i < b->len) ? b->limbs[i] : 0;
        uint64_t s = va + vb + carry;
        out->limbs[out->len++] = (uint32_t)(s % ZR_BI_BASE);
        carry = s / ZR_BI_BASE;
    }
    if (carry > 0) out->limbs[out->len++] = (uint32_t)carry;
    out->sign = 1;
    zr_bi_trim(out);
}

/* Assumes |a| >= |b| */
static void zr_bi_sub_abs(ZrBigInt *out, const ZrBigInt *a, const ZrBigInt *b) {
    zr_bi_init(out);
    if (a->len == 0) return;
    zr_bi_reserve(out, a->len);
    int64_t borrow = 0;
    for (size_t i = 0; i < a->len; i++) {
        int64_t va = a->limbs[i];
        int64_t vb = (i < b->len) ? b->limbs[i] : 0;
        int64_t d = va - vb - borrow;
        if (d < 0) {
            d += ZR_BI_BASE;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out->limbs[out->len++] = (uint32_t)d;
    }
    out->sign = 1;
    zr_bi_trim(out);
}

static void zr_bi_add(ZrBigInt *out, const ZrBigInt *a, const ZrBigInt *b) {
    if (a->sign == 0) { zr_bi_copy(out, b); return; }
    if (b->sign == 0) { zr_bi_copy(out, a); return; }
    if (a->sign == b->sign) {
        zr_bi_add_abs(out, a, b);
        if (out->sign != 0) out->sign = a->sign;
    } else {
        int c = zr_bi_cmp_abs(a, b);
        if (c == 0) {
            zr_bi_init(out);
        } else if (c > 0) {
            zr_bi_sub_abs(out, a, b);
            out->sign = a->sign;
        } else {
            zr_bi_sub_abs(out, b, a);
            out->sign = b->sign;
        }
    }
}

static void zr_bi_sub(ZrBigInt *out, const ZrBigInt *a, const ZrBigInt *b) {
    ZrBigInt nb = *b;
    nb.sign = -nb.sign;
    zr_bi_add(out, a, &nb);
}

static void zr_bi_mul(ZrBigInt *out, const ZrBigInt *a, const ZrBigInt *b) {
    zr_bi_init(out);
    if (a->sign == 0 || b->sign == 0) return;
    zr_bi_reserve(out, a->len + b->len);
    memset(out->limbs, 0, (a->len + b->len) * sizeof(uint32_t));
    out->len = a->len + b->len;
    for (size_t i = 0; i < a->len; i++) {
        uint64_t carry = 0;
        uint64_t ai = a->limbs[i];
        for (size_t j = 0; j < b->len; j++) {
            uint64_t cur = out->limbs[i + j] + ai * (uint64_t)b->limbs[j] + carry;
            out->limbs[i + j] = (uint32_t)(cur % ZR_BI_BASE);
            carry = cur / ZR_BI_BASE;
        }
        size_t k = i + b->len;
        while (carry > 0) {
            uint64_t cur = out->limbs[k] + carry;
            out->limbs[k] = (uint32_t)(cur % ZR_BI_BASE);
            carry = cur / ZR_BI_BASE;
            k++;
        }
    }
    out->sign = (a->sign == b->sign) ? 1 : -1;
    zr_bi_trim(out);
}

/* Exact division & remainder in base 10^9: a = q * b + r (truncated toward zero) */
static bool zr_bi_divmod(ZrBigInt *q_out, ZrBigInt *r_out, const ZrBigInt *a, const ZrBigInt *b) {
    zr_bi_init(q_out);
    zr_bi_init(r_out);
    if (b->sign == 0 || b->len == 0) return false;
    if (a->sign == 0) return true;
    int cmp = zr_bi_cmp_abs(a, b);
    if (cmp < 0) {
        zr_bi_copy(r_out, a);
        return true;
    }
    ZrBigInt abs_b;
    zr_bi_copy(&abs_b, b);
    abs_b.sign = 1;

    zr_bi_reserve(q_out, a->len);
    memset(q_out->limbs, 0, a->len * sizeof(uint32_t));
    q_out->len = a->len;

    ZrBigInt rem;
    zr_bi_init(&rem);

    for (size_t idx = a->len; idx > 0; idx--) {
        uint32_t digit = a->limbs[idx - 1];
        if (rem.sign != 0) {
            zr_bi_reserve(&rem, rem.len + 1);
            memmove(rem.limbs + 1, rem.limbs, rem.len * sizeof(uint32_t));
            rem.limbs[0] = digit;
            rem.len++;
        } else if (digit != 0) {
            zr_bi_reserve(&rem, 1);
            rem.limbs[0] = digit;
            rem.len = 1;
            rem.sign = 1;
        }
        if (rem.sign == 0 || zr_bi_cmp_abs(&rem, &abs_b) < 0) {
            q_out->limbs[idx - 1] = 0;
            continue;
        }
        uint32_t lo = 1, hi = ZR_BI_BASE - 1, best = 1;
        while (lo <= hi) {
            uint32_t mid = lo + (hi - lo) / 2;
            ZrBigInt prod;
            zr_bi_copy(&prod, &abs_b);
            zr_bi_mul_u32_ip(&prod, mid);
            int c = zr_bi_cmp_abs(&prod, &rem);
            zr_bi_free(&prod);
            if (c <= 0) {
                best = mid;
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        q_out->limbs[idx - 1] = best;
        ZrBigInt prod, nrem;
        zr_bi_copy(&prod, &abs_b);
        zr_bi_mul_u32_ip(&prod, best);
        zr_bi_sub_abs(&nrem, &rem, &prod);
        zr_bi_free(&prod);
        zr_bi_free(&rem);
        rem = nrem;
    }

    zr_bi_free(&abs_b);
    q_out->sign = (a->sign == b->sign) ? 1 : -1;
    zr_bi_trim(q_out);
    *r_out = rem;
    if (r_out->sign != 0) r_out->sign = a->sign;
    return true;
}

static bool zr_bi_pow(ZrBigInt *out, const ZrBigInt *base, uint64_t exp) {
    if (exp > 1000000ULL) return false;
    ZrBigInt res, cur;
    zr_bi_from_i64(&res, 1);
    zr_bi_copy(&cur, base);
    uint64_t e = exp;
    while (e > 0) {
        if (e & 1ULL) {
            ZrBigInt tmp;
            zr_bi_mul(&tmp, &res, &cur);
            zr_bi_free(&res);
            res = tmp;
        }
        e >>= 1;
        if (e > 0) {
            ZrBigInt tmp;
            zr_bi_mul(&tmp, &cur, &cur);
            zr_bi_free(&cur);
            cur = tmp;
        }
    }
    zr_bi_free(&cur);
    *out = res;
    return true;
}

static void zr_bi_gcd(ZrBigInt *out, const ZrBigInt *a, const ZrBigInt *b) {
    ZrBigInt x, y;
    zr_bi_copy(&x, a); x.sign = x.len ? 1 : 0;
    zr_bi_copy(&y, b); y.sign = y.len ? 1 : 0;
    while (y.sign != 0) {
        ZrBigInt q, r;
        zr_bi_divmod(&q, &r, &x, &y);
        zr_bi_free(&q);
        zr_bi_free(&x);
        x = y;
        y = r;
    }
    zr_bi_free(&y);
    *out = x;
}

/* ---- Rational Helpers ---- */

static inline void zr_rat_free(ZrRational *r) {
    zr_bi_free(&r->num);
    zr_bi_free(&r->den);
}

static bool zr_rat_init_norm(ZrRational *r, const ZrBigInt *num, const ZrBigInt *den) {
    if (den->sign == 0) return false;
    if (num->sign == 0) {
        zr_bi_from_i64(&r->num, 0);
        zr_bi_from_i64(&r->den, 1);
        return true;
    }
    ZrBigInt g, qn, rn, qd, rd;
    zr_bi_gcd(&g, num, den);
    zr_bi_divmod(&qn, &rn, num, &g);
    zr_bi_divmod(&qd, &rd, den, &g);
    zr_bi_free(&g); zr_bi_free(&rn); zr_bi_free(&rd);
    if (qd.sign < 0) {
        qd.sign = 1;
        qn.sign = -qn.sign;
    }
    r->num = qn;
    r->den = qd;
    return true;
}

static inline bool zr_rat_is_int(const ZrRational *r) {
    return r->den.sign == 1 && r->den.len == 1 && r->den.limbs[0] == 1;
}

static char *zr_rat_to_str(const ZrRational *r) {
    if (zr_rat_is_int(r)) return zr_bi_to_str(&r->num);
    char *ns = zr_bi_to_str(&r->num);
    char *ds = zr_bi_to_str(&r->den);
    ZrBuf b;
    zr_buf_init(&b);
    zr_buf_printf(&b, "%s/%s", ns, ds);
    free(ns);
    free(ds);
    return zr_buf_take(&b, NULL);
}

static bool zr_rat_from_str(ZrRational *r, const char *s) {
    const char *slash = strchr(s, '/');
    if (slash) {
        char *lhs = zr_xstrndup(s, (size_t)(slash - s));
        ZrBigInt n, d;
        bool ok = zr_bi_from_str(&n, lhs) && zr_bi_from_str(&d, slash + 1);
        free(lhs);
        if (!ok) { zr_bi_free(&n); zr_bi_free(&d); return false; }
        bool r_ok = zr_rat_init_norm(r, &n, &d);
        zr_bi_free(&n); zr_bi_free(&d);
        return r_ok;
    }
    const char *dot = strchr(s, '.');
    if (dot) {
        ZrBuf digits;
        zr_buf_init(&digits);
        const char *p = s;
        while (*p && isspace((unsigned char)*p)) p++;
        if (*p == '-' || *p == '+') zr_buf_push(&digits, *p++);
        uint64_t frac_digits = 0;
        bool seen_dot = false;
        while (*p && !isspace((unsigned char)*p)) {
            if (*p == '.') {
                if (seen_dot) { zr_buf_free(&digits); return false; }
                seen_dot = true;
            } else if (*p >= '0' && *p <= '9') {
                zr_buf_push(&digits, *p);
                if (seen_dot) frac_digits++;
            } else if (*p != '_') {
                zr_buf_free(&digits);
                return false;
            }
            p++;
        }
        ZrBigInt n, ten, d;
        bool ok = zr_bi_from_str(&n, digits.data ? digits.data : "");
        zr_buf_free(&digits);
        if (!ok) return false;
        zr_bi_from_i64(&ten, 10);
        zr_bi_pow(&d, &ten, frac_digits);
        zr_bi_free(&ten);
        bool r_ok = zr_rat_init_norm(r, &n, &d);
        zr_bi_free(&n); zr_bi_free(&d);
        return r_ok;
    }
    ZrBigInt n, d;
    if (!zr_bi_from_str(&n, s)) return false;
    zr_bi_from_i64(&d, 1);
    bool r_ok = zr_rat_init_norm(r, &n, &d);
    zr_bi_free(&n); zr_bi_free(&d);
    return r_ok;
}

/* ---- Decimal Helpers ---- */

static inline void zr_dec_free(ZrDecimal *d) {
    zr_bi_free(&d->coeff);
    d->scale = 0;
}

static void zr_dec_normalize(ZrDecimal *d) {
    if (d->coeff.sign == 0) {
        d->scale = 0;
        return;
    }
    ZrBigInt ten;
    zr_bi_from_i64(&ten, 10);
    while (d->scale > 0) {
        ZrBigInt q, r;
        zr_bi_divmod(&q, &r, &d->coeff, &ten);
        if (r.sign == 0) {
            zr_bi_free(&r);
            zr_bi_free(&d->coeff);
            d->coeff = q;
            d->scale--;
        } else {
            zr_bi_free(&q);
            zr_bi_free(&r);
            break;
        }
    }
    zr_bi_free(&ten);
}

static bool zr_dec_from_str(ZrDecimal *d, const char *s) {
    d->scale = 0;
    zr_bi_init(&d->coeff);
    if (!s) return false;
    while (*s && isspace((unsigned char)*s)) s++;
    int sign = 1;
    if (*s == '-') { sign = -1; s++; }
    else if (*s == '+') { s++; }
    ZrBuf digits;
    zr_buf_init(&digits);
    bool seen_dot = false;
    int32_t scale = 0;
    while (*s && !isspace((unsigned char)*s) && *s != 'e' && *s != 'E') {
        if (*s == '.') {
            if (seen_dot) { zr_buf_free(&digits); return false; }
            seen_dot = true;
        } else if (*s >= '0' && *s <= '9') {
            zr_buf_push(&digits, *s);
            if (seen_dot) scale++;
        } else if (*s != '_') {
            zr_buf_free(&digits);
            return false;
        }
        s++;
    }
    if (*s == 'e' || *s == 'E') {
        s++;
        int exp_val = atoi(s);
        scale -= exp_val;
    }
    if (digits.len == 0) {
        zr_buf_free(&digits);
        return false;
    }
    bool ok = zr_bi_from_str(&d->coeff, digits.data);
    zr_buf_free(&digits);
    if (!ok) return false;
    if (d->coeff.sign != 0) d->coeff.sign = sign;
    while (scale < 0) {
        zr_bi_mul_u32_ip(&d->coeff, 10);
        scale++;
    }
    d->scale = scale;
    zr_dec_normalize(d);
    return true;
}

static char *zr_dec_to_str(const ZrDecimal *d) {
    if (d->coeff.sign == 0) return zr_xstrdup("0");
    ZrBigInt abs_c;
    zr_bi_copy(&abs_c, &d->coeff);
    abs_c.sign = 1;
    char *raw = zr_bi_to_str(&abs_c);
    zr_bi_free(&abs_c);
    size_t rlen = strlen(raw);
    ZrBuf b;
    zr_buf_init(&b);
    if (d->coeff.sign < 0) zr_buf_push(&b, '-');
    if (d->scale <= 0) {
        zr_buf_append(&b, raw, rlen);
    } else if ((size_t)d->scale >= rlen) {
        zr_buf_append_cstr(&b, "0.");
        for (size_t z = 0; z < (size_t)d->scale - rlen; z++) zr_buf_push(&b, '0');
        zr_buf_append(&b, raw, rlen);
    } else {
        size_t int_part = rlen - (size_t)d->scale;
        zr_buf_append(&b, raw, int_part);
        zr_buf_push(&b, '.');
        zr_buf_append(&b, raw + int_part, (size_t)d->scale);
    }
    free(raw);
    return zr_buf_take(&b, NULL);
}

/* ---- Cross-Header Compatibility Shims ---- */

#ifndef ZR_PATH_SEP
#ifdef _WIN32
#define ZR_PATH_SEP '\\'
#else
#define ZR_PATH_SEP '/'
#endif
#endif

typedef void *ZrDlHandle;

static inline char *zr_strdup(const char *s) { return zr_xstrdup(s); }
static inline double zr_monotonic_sec(void) { return zr_mono_sec(); }
static inline int zr_hex_nibble(char c) { return zr_hex_val(c); }
static inline bool zr_csprng_bytes(uint8_t *buf, size_t len) { return zr_csprng_fill(buf, len); }
static inline char *zr_base64_encode(const uint8_t *src, size_t len, size_t *out_len) { return zr_b64_encode(src, len, out_len); }
static inline bool zr_base64_decode(const char *src, size_t len, uint8_t **out_buf, size_t *out_len) { return zr_b64_decode(src, len, out_buf, out_len); }
static inline void zr_sha1_oneshot(const void *msg, size_t len, uint8_t out[20]) { zr_sha1((const uint8_t *)msg, len, out); }
static inline void zr_sha256_oneshot(const void *msg, size_t len, uint8_t out[32]) { zr_sha256((const uint8_t *)msg, len, out); }
static inline void zr_sha512_oneshot(const void *msg, size_t len, uint8_t out[64]) { zr_sha512((const uint8_t *)msg, len, out); }

typedef struct {
    ZrBuf buf;
} ZrSha256Ctx;

static inline void zr_sha256_init(ZrSha256Ctx *ctx) { zr_buf_init(&ctx->buf); }
static inline void zr_sha256_update(ZrSha256Ctx *ctx, const void *data, size_t len) { zr_buf_append(&ctx->buf, data, len); }
static inline void zr_sha256_final(ZrSha256Ctx *ctx, uint8_t out[32]) {
    zr_sha256((const uint8_t *)(ctx->buf.data ? ctx->buf.data : ""), ctx->buf.len, out);
    zr_buf_free(&ctx->buf);
}

/* ---- v3.0.0 Atomic Refcount, Nanosecond Timer & Case-Insensitive Helpers ---- */

static inline int32_t zr_atomic_inc32(volatile int32_t *p) {
#if defined(_WIN32)
    return (int32_t)InterlockedIncrement((volatile LONG *)p);
#elif defined(__GNUC__) || defined(__clang__)
    return __sync_add_and_fetch(p, 1);
#else
    return ++(*p);
#endif
}

static inline int32_t zr_atomic_dec32(volatile int32_t *p) {
#if defined(_WIN32)
    return (int32_t)InterlockedDecrement((volatile LONG *)p);
#elif defined(__GNUC__) || defined(__clang__)
    return __sync_sub_and_fetch(p, 1);
#else
    return --(*p);
#endif
}

static inline uint64_t zr_mono_ns(void) {
#ifdef _WIN32
    static LARGE_INTEGER freq = {0};
    if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    return (uint64_t)(((double)now.QuadPart * 1000000000.0) / (double)freq.QuadPart);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
#endif
}

static inline bool zr_str_contains_ci_ascii(const char *hay, const char *needle) {
    if (!hay || !needle) return false;
    size_t nlen = strlen(needle);
    if (nlen == 0) return true;
    for (const char *p = hay; *p; p++) {
        size_t i = 0;
        while (i < nlen && p[i]) {
            char a = p[i], b = needle[i];
            if (a >= 'a' && a <= 'z') a = (char)(a - 32);
            if (b >= 'a' && b <= 'z') b = (char)(b - 32);
            if (a != b) break;
            i++;
        }
        if (i == nlen) return true;
    }
    return false;
}

static inline int zr_strcasecmp_ascii(const char *a, const char *b) {
    if (!a) a = "";
    if (!b) b = "";
    while (*a && *b) {
        unsigned char ca = (unsigned char)*a++;
        unsigned char cb = (unsigned char)*b++;
        if (ca >= 'A' && ca <= 'Z') ca = (unsigned char)(ca + 32);
        if (cb >= 'A' && cb <= 'Z') cb = (unsigned char)(cb + 32);
        if (ca != cb) return (int)ca - (int)cb;
    }
    unsigned char ca = (unsigned char)*a;
    unsigned char cb = (unsigned char)*b;
    if (ca >= 'A' && ca <= 'Z') ca = (unsigned char)(ca + 32);
    if (cb >= 'A' && cb <= 'Z') cb = (unsigned char)(cb + 32);
    return (int)ca - (int)cb;
}

static inline int zr_strncasecmp_ascii(const char *a, const char *b, size_t n) {
    if (!a) a = "";
    if (!b) b = "";
    for (size_t i = 0; i < n; i++) {
        unsigned char ca = (unsigned char)a[i];
        unsigned char cb = (unsigned char)b[i];
        if (ca >= 'A' && ca <= 'Z') ca = (unsigned char)(ca + 32);
        if (cb >= 'A' && cb <= 'Z') cb = (unsigned char)(cb + 32);
        if (ca != cb || ca == 0) return (int)ca - (int)cb;
    }
    return 0;
}

#endif /* PBLANG_ZR_COMMON_H */
