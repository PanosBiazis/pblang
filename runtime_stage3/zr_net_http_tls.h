/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v3.0.0 — zr_net_http_tls.h
 * Thread-safe TCP sockets, poll readiness, lazy-dlopen OpenSSL TLS 1.3 (Linux) /
 * WinHTTP (Windows), HTTP/1.1 client + server helpers, SSE, and RFC 6455 WS.
 * ============================================================================ */
#ifndef PBLANG_ZR_NET_HTTP_TLS_H
#define PBLANG_ZR_NET_HTTP_TLS_H

#include "zr_json_crypto.h"

#if defined(_WIN32)
#include <winhttp.h>
typedef SOCKET ZrSock;
#define ZR_INVALID_SOCK INVALID_SOCKET
static void zr_close_raw_sock(ZrSock s) { if (s != INVALID_SOCKET) closesocket(s); }
static bool g_zr_wsa_ready = false;
static inline void zr_ensure_winsock(void) {
    if (!g_zr_wsa_ready) {
        WSADATA wd;
        if (WSAStartup(MAKEWORD(2, 2), &wd) == 0) g_zr_wsa_ready = true;
    }
}
#else
typedef int ZrSock;
#define ZR_INVALID_SOCK (-1)
static void zr_close_raw_sock(ZrSock s) { if (s >= 0) close(s); }
static inline void zr_ensure_winsock(void) {}
#endif

/* ---- Lazy-loaded OpenSSL 3.x / 1.1.1 Function Table ---- */
typedef struct ssl_method_st ZrSslMethod;
typedef struct ssl_ctx_st    ZrSslCtx;
typedef struct ssl_st        ZrSsl;

#define ZR_SSL_FILETYPE_PEM             1
#define ZR_SSL_VERIFY_NONE              0x00
#define ZR_SSL_VERIFY_PEER              0x01
#define ZR_SSL_CTRL_SET_TLSEXT_HOSTNAME 55
#define ZR_SSL_CTRL_SET_MIN_PROTO_VER   123
#define ZR_TLS1_2_VERSION               0x0303
#define ZR_X509_V_OK                    0

typedef struct {
    bool      tried;
    bool      ready;
    ZrDlHandle libssl;
    ZrDlHandle libcrypto;

    int               (*OPENSSL_init_ssl)(uint64_t, const void *);
    const ZrSslMethod *(*TLS_client_method)(void);
    const ZrSslMethod *(*TLS_server_method)(void);
    ZrSslCtx          *(*SSL_CTX_new)(const ZrSslMethod *);
    void               (*SSL_CTX_free)(ZrSslCtx *);
    int                (*SSL_CTX_set_default_verify_paths)(ZrSslCtx *);
    int                (*SSL_CTX_load_verify_locations)(ZrSslCtx *, const char *, const char *);
    void               (*SSL_CTX_set_verify)(ZrSslCtx *, int, void *);
    int                (*SSL_CTX_use_certificate_chain_file)(ZrSslCtx *, const char *);
    int                (*SSL_CTX_use_PrivateKey_file)(ZrSslCtx *, const char *, int);
    int                (*SSL_CTX_check_private_key)(const ZrSslCtx *);
    ZrSsl             *(*SSL_new)(ZrSslCtx *);
    void               (*SSL_free)(ZrSsl *);
    int                (*SSL_set_fd)(ZrSsl *, int);
    long               (*SSL_ctrl)(ZrSsl *, int, long, void *);
    long               (*SSL_CTX_ctrl)(ZrSslCtx *, int, long, void *);
    int                (*SSL_set1_host)(ZrSsl *, const char *);
    int                (*SSL_connect)(ZrSsl *);
    int                (*SSL_accept)(ZrSsl *);
    int                (*SSL_read)(ZrSsl *, void *, int);
    int                (*SSL_write)(ZrSsl *, const void *, int);
    int                (*SSL_shutdown)(ZrSsl *);
    long               (*SSL_get_verify_result)(const ZrSsl *);
    unsigned long      (*ERR_get_error)(void);
    void               (*ERR_error_string_n)(unsigned long, char *, size_t);
} ZrOpenSslApi;

static ZrOpenSslApi g_zr_ssl = {0};
static ZrMutex      g_zr_net_mtx;
static bool         g_zr_net_mtx_init = false;

static void zr_net_subsystem_init(void) {
    if (!g_zr_net_mtx_init) {
        zr_mutex_init(&g_zr_net_mtx);
        g_zr_net_mtx_init = true;
    }
    zr_net_ensure_init();
#if !defined(_WIN32) && defined(SIGPIPE)
    signal(SIGPIPE, SIG_IGN);
#endif
}

static bool zr_ensure_openssl(char err_out[256]) {
    zr_net_subsystem_init();
    zr_mutex_lock(&g_zr_net_mtx);
    if (g_zr_ssl.tried) {
        bool ok = g_zr_ssl.ready;
        zr_mutex_unlock(&g_zr_net_mtx);
        if (!ok) snprintf(err_out, 256, "OpenSSL shared library (libssl) is unavailable on this host");
        return ok;
    }
    g_zr_ssl.tried = true;

#if defined(_WIN32)
    const char *ssl_cands[] = { "libssl-3-x64.dll", "libssl-1_1-x64.dll", "libssl.dll", NULL };
    const char *cry_cands[] = { "libcrypto-3-x64.dll", "libcrypto-1_1-x64.dll", "libcrypto.dll", NULL };
#elif defined(__APPLE__)
    const char *ssl_cands[] = { "libssl.3.dylib", "/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib", "libssl.dylib", NULL };
    const char *cry_cands[] = { "libcrypto.3.dylib", "/opt/homebrew/opt/openssl@3/lib/libcrypto.3.dylib", "libcrypto.dylib", NULL };
#else
    const char *ssl_cands[] = {
        "libssl.so.3",
        "/usr/lib/x86_64-linux-gnu/libssl.so.3",
        "/lib/x86_64-linux-gnu/libssl.so.3",
        "/usr/lib/aarch64-linux-gnu/libssl.so.3",
        "libssl.so.1.1",
        "libssl.so",
        NULL
    };
    const char *cry_cands[] = {
        "libcrypto.so.3",
        "/usr/lib/x86_64-linux-gnu/libcrypto.so.3",
        "/lib/x86_64-linux-gnu/libcrypto.so.3",
        "/usr/lib/aarch64-linux-gnu/libcrypto.so.3",
        "libcrypto.so.1.1",
        "libcrypto.so",
        NULL
    };
#endif

    for (int i = 0; cry_cands[i] && !g_zr_ssl.libcrypto; i++) g_zr_ssl.libcrypto = zr_dlopen(cry_cands[i]);
    for (int i = 0; ssl_cands[i] && !g_zr_ssl.libssl; i++)    g_zr_ssl.libssl    = zr_dlopen(ssl_cands[i]);

    if (!g_zr_ssl.libssl) {
        zr_mutex_unlock(&g_zr_net_mtx);
        snprintf(err_out, 256, "OpenSSL libssl shared library not found");
        return false;
    }

    #define LOAD_SSL_SYM(fn) g_zr_ssl.fn = (__typeof__(g_zr_ssl.fn))zr_dlsym(g_zr_ssl.libssl, #fn)
    LOAD_SSL_SYM(OPENSSL_init_ssl);
    LOAD_SSL_SYM(TLS_client_method);
    LOAD_SSL_SYM(TLS_server_method);
    LOAD_SSL_SYM(SSL_CTX_new);
    LOAD_SSL_SYM(SSL_CTX_free);
    LOAD_SSL_SYM(SSL_CTX_set_default_verify_paths);
    LOAD_SSL_SYM(SSL_CTX_load_verify_locations);
    LOAD_SSL_SYM(SSL_CTX_set_verify);
    LOAD_SSL_SYM(SSL_CTX_use_certificate_chain_file);
    LOAD_SSL_SYM(SSL_CTX_use_PrivateKey_file);
    LOAD_SSL_SYM(SSL_CTX_check_private_key);
    LOAD_SSL_SYM(SSL_new);
    LOAD_SSL_SYM(SSL_free);
    LOAD_SSL_SYM(SSL_set_fd);
    LOAD_SSL_SYM(SSL_ctrl);
    LOAD_SSL_SYM(SSL_CTX_ctrl);
    LOAD_SSL_SYM(SSL_set1_host);
    LOAD_SSL_SYM(SSL_connect);
    LOAD_SSL_SYM(SSL_accept);
    LOAD_SSL_SYM(SSL_read);
    LOAD_SSL_SYM(SSL_write);
    LOAD_SSL_SYM(SSL_shutdown);
    LOAD_SSL_SYM(SSL_get_verify_result);
    #undef LOAD_SSL_SYM

    if (g_zr_ssl.libcrypto) {
        g_zr_ssl.ERR_get_error = (__typeof__(g_zr_ssl.ERR_get_error))zr_dlsym(g_zr_ssl.libcrypto, "ERR_get_error");
        g_zr_ssl.ERR_error_string_n = (__typeof__(g_zr_ssl.ERR_error_string_n))zr_dlsym(g_zr_ssl.libcrypto, "ERR_error_string_n");
    }

    if (!g_zr_ssl.TLS_client_method || !g_zr_ssl.TLS_server_method || !g_zr_ssl.SSL_CTX_new ||
        !g_zr_ssl.SSL_new || !g_zr_ssl.SSL_set_fd || !g_zr_ssl.SSL_connect ||
        !g_zr_ssl.SSL_accept || !g_zr_ssl.SSL_read || !g_zr_ssl.SSL_write) {
        zr_mutex_unlock(&g_zr_net_mtx);
        snprintf(err_out, 256, "OpenSSL library missing required TLS 1.3 entry points");
        return false;
    }

    if (g_zr_ssl.OPENSSL_init_ssl) g_zr_ssl.OPENSSL_init_ssl(0, NULL);
    g_zr_ssl.ready = true;
    zr_mutex_unlock(&g_zr_net_mtx);
    return true;
}

static void zr_ssl_last_err(char buf[256]) {
    if (g_zr_ssl.ERR_get_error && g_zr_ssl.ERR_error_string_n) {
        unsigned long e = g_zr_ssl.ERR_get_error();
        if (e != 0) {
            g_zr_ssl.ERR_error_string_n(e, buf, 256);
            return;
        }
    }
    snprintf(buf, 256, "TLS handshake or I/O error");
}

/* ---- Thread-Safe Global Net Handle Table ---- */
typedef enum {
    ZR_NET_NONE = 0,
    ZR_NET_LISTENER,
    ZR_NET_TLS_LISTENER,
    ZR_NET_STREAM,
    ZR_NET_UDP
} ZrNetKind;

typedef struct {
    size_t    id;
    ZrNetKind kind;
    bool      busy;
    ZrSock    sock;
    ZrSslCtx *ssl_ctx;
    ZrSsl    *ssl;
    ZrBuf     rx_buf;
} ZrNetEntry;

#define ZR_MAX_NET_HANDLES 2048
static ZrNetEntry g_zr_net_table[ZR_MAX_NET_HANDLES];
static size_t     g_zr_net_next_id = 1;

static void zr_set_sock_timeouts(ZrSock s, uint64_t timeout_ms) {
    if (timeout_ms == 0) return;
#if defined(_WIN32)
    DWORD tv = (DWORD)timeout_ms;
    setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, (const char *)&tv, sizeof(tv));
    setsockopt(s, SOL_SOCKET, SO_SNDTIMEO, (const char *)&tv, sizeof(tv));
#elif defined(__wasi__)
    (void)s;
#else
    struct timeval tv;
    tv.tv_sec  = (time_t)(timeout_ms / 1000ULL);
    tv.tv_usec = (suseconds_t)((timeout_ms % 1000ULL) * 1000ULL);
    setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    setsockopt(s, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
#endif
}

static size_t zr_net_insert(ZrNetKind kind, ZrSock sock, ZrSslCtx *ctx, ZrSsl *ssl) {
    zr_net_subsystem_init();
    zr_mutex_lock(&g_zr_net_mtx);
    size_t id = g_zr_net_next_id++;
    for (size_t i = 0; i < ZR_MAX_NET_HANDLES; i++) {
        if (g_zr_net_table[i].kind == ZR_NET_NONE) {
            g_zr_net_table[i].id = id;
            g_zr_net_table[i].kind = kind;
            g_zr_net_table[i].busy = false;
            g_zr_net_table[i].sock = sock;
            g_zr_net_table[i].ssl_ctx = ctx;
            g_zr_net_table[i].ssl = ssl;
            zr_buf_init(&g_zr_net_table[i].rx_buf);
            zr_mutex_unlock(&g_zr_net_mtx);
            return id;
        }
    }
    zr_mutex_unlock(&g_zr_net_mtx);
    return 0;
}

static ZrNetEntry *zr_net_checkout(size_t id) {
    zr_net_subsystem_init();
    zr_mutex_lock(&g_zr_net_mtx);
    for (size_t i = 0; i < ZR_MAX_NET_HANDLES; i++) {
        if (g_zr_net_table[i].kind != ZR_NET_NONE && g_zr_net_table[i].id == id && !g_zr_net_table[i].busy) {
            g_zr_net_table[i].busy = true;
            zr_mutex_unlock(&g_zr_net_mtx);
            return &g_zr_net_table[i];
        }
    }
    zr_mutex_unlock(&g_zr_net_mtx);
    return NULL;
}

static void zr_net_checkin(ZrNetEntry *e) {
    zr_mutex_lock(&g_zr_net_mtx);
    if (e) e->busy = false;
    zr_mutex_unlock(&g_zr_net_mtx);
}

static void zr_net_destroy_entry(ZrNetEntry *e) {
    if (!e || e->kind == ZR_NET_NONE) return;
    if (e->ssl && g_zr_ssl.ready) {
        if (g_zr_ssl.SSL_shutdown) g_zr_ssl.SSL_shutdown(e->ssl);
        g_zr_ssl.SSL_free(e->ssl);
    }
    if (e->ssl_ctx && g_zr_ssl.ready) {
        g_zr_ssl.SSL_CTX_free(e->ssl_ctx);
    }
    zr_close_raw_sock(e->sock);
    zr_buf_free(&e->rx_buf);
    memset(e, 0, sizeof(*e));
    e->sock = ZR_INVALID_SOCK;
}

/* Split "host:port" or "[::1]:port" */
static bool zr_split_host_port(const char *addr, char host[512], char port[32]) {
    if (!addr || !addr[0]) return false;
    if (addr[0] == '[') {
        const char *rb = strchr(addr, ']');
        if (!rb || rb[1] != ':') return false;
        size_t hlen = (size_t)(rb - addr - 1);
        if (hlen >= 512) return false;
        memcpy(host, addr + 1, hlen);
        host[hlen] = '\0';
        snprintf(port, 32, "%s", rb + 2);
        return port[0] != '\0';
    }
    const char *colon = strrchr(addr, ':');
    if (!colon || colon == addr) return false;
    size_t hlen = (size_t)(colon - addr);
    if (hlen >= 512) return false;
    memcpy(host, addr, hlen);
    host[hlen] = '\0';
    snprintf(port, 32, "%s", colon + 1);
    return port[0] != '\0';
}

static ZrSock zr_bind_tcp_listener(const char *addr, char err_out[256]) {
#if defined(__wasi__)
    (void)addr;
    snprintf(err_out, 256, "raw TCP listeners unavailable in WASI preview1 sandbox");
    return ZR_INVALID_SOCK;
#else
    zr_net_ensure_init();
    char host[512], port[32];
    if (!zr_split_host_port(addr, host, port)) {
        snprintf(err_out, 256, "invalid address '%s' (expected host:port)", addr);
        return ZR_INVALID_SOCK;
    }
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = AI_PASSIVE;
    if (getaddrinfo(host[0] ? host : NULL, port, &hints, &res) != 0 || !res) {
        snprintf(err_out, 256, "getaddrinfo failed for '%s'", addr);
        return ZR_INVALID_SOCK;
    }
    ZrSock s = ZR_INVALID_SOCK;
    for (struct addrinfo *ai = res; ai; ai = ai->ai_next) {
        s = socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol);
        if (s == ZR_INVALID_SOCK) continue;
        int opt = 1;
        setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));
        if (bind(s, ai->ai_addr, (int)ai->ai_addrlen) == 0 && listen(s, 128) == 0) break;
        zr_close_raw_sock(s);
        s = ZR_INVALID_SOCK;
    }
    freeaddrinfo(res);
    if (s == ZR_INVALID_SOCK) snprintf(err_out, 256, "bind/listen failed for '%s'", addr);
    return s;
#endif
}

static ZrSock zr_connect_tcp_raw(const char *host, const char *port, uint64_t timeout_ms, char err_out[256]) {
#if defined(__wasi__)
    (void)host; (void)port; (void)timeout_ms;
    snprintf(err_out, 256, "raw TCP sockets unavailable in WASI preview1 sandbox");
    return ZR_INVALID_SOCK;
#else
    zr_net_ensure_init();
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    if (getaddrinfo(host, port, &hints, &res) != 0 || !res) {
        snprintf(err_out, 256, "DNS lookup failed for '%s:%s'", host, port);
        return ZR_INVALID_SOCK;
    }
    ZrSock s = ZR_INVALID_SOCK;
    for (struct addrinfo *ai = res; ai; ai = ai->ai_next) {
        s = socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol);
        if (s == ZR_INVALID_SOCK) continue;
        zr_set_sock_timeouts(s, timeout_ms ? timeout_ms : 15000ULL);
        if (connect(s, ai->ai_addr, (int)ai->ai_addrlen) == 0) break;
        zr_close_raw_sock(s);
        s = ZR_INVALID_SOCK;
    }
    freeaddrinfo(res);
    if (s == ZR_INVALID_SOCK) snprintf(err_out, 256, "connect failed to '%s:%s'", host, port);
    return s;
#endif
}

static ssize_t zr_stream_read_some(ZrNetEntry *e, uint8_t *buf, size_t cap, char err_out[256]) {
    if (e->ssl) {
        int n = g_zr_ssl.SSL_read(e->ssl, buf, (int)cap);
        if (n < 0) { zr_ssl_last_err(err_out); return -1; }
        return (ssize_t)n;
    }
#if defined(_WIN32)
    int n = recv(e->sock, (char *)buf, (int)cap, 0);
    if (n < 0) { snprintf(err_out, 256, "recv failed (%d)", WSAGetLastError()); return -1; }
    return (ssize_t)n;
#elif defined(__wasi__)
    (void)buf; (void)cap;
    snprintf(err_out, 256, "recv unavailable in WASI");
    return -1;
#else
    ssize_t n;
    do { n = recv(e->sock, buf, cap, 0); } while (n < 0 && errno == EINTR);
    if (n < 0) { snprintf(err_out, 256, "%s", strerror(errno)); return -1; }
    return n;
#endif
}

static ssize_t zr_stream_write_all(ZrNetEntry *e, const uint8_t *buf, size_t len, char err_out[256]) {
    size_t sent = 0;
    while (sent < len) {
        if (e->ssl) {
            int n = g_zr_ssl.SSL_write(e->ssl, buf + sent, (int)(len - sent));
            if (n <= 0) { zr_ssl_last_err(err_out); return -1; }
            sent += (size_t)n;
        } else {
#if defined(_WIN32)
            int n = send(e->sock, (const char *)(buf + sent), (int)(len - sent), 0);
            if (n <= 0) { snprintf(err_out, 256, "send failed (%d)", WSAGetLastError()); return -1; }
            sent += (size_t)n;
#elif defined(__wasi__)
            (void)buf;
            snprintf(err_out, 256, "send unavailable in WASI");
            return -1;
#else
            ssize_t n;
            do { n = send(e->sock, buf + sent, len - sent, 0); } while (n < 0 && errno == EINTR);
            if (n <= 0) { snprintf(err_out, 256, "%s", strerror(errno)); return -1; }
            sent += (size_t)n;
#endif
        }
    }
    return (ssize_t)sent;
}

static ZrSslCtx *zr_build_server_ssl_ctx(const char *cert_path, const char *key_path, char err_out[256]) {
    if (!zr_ensure_openssl(err_out)) return NULL;
    ZrSslCtx *ctx = g_zr_ssl.SSL_CTX_new(g_zr_ssl.TLS_server_method());
    if (!ctx) { zr_ssl_last_err(err_out); return NULL; }
    if (g_zr_ssl.SSL_CTX_ctrl) {
        g_zr_ssl.SSL_CTX_ctrl(ctx, ZR_SSL_CTRL_SET_MIN_PROTO_VER, ZR_TLS1_2_VERSION, NULL);
    }
    if (g_zr_ssl.SSL_CTX_use_certificate_chain_file(ctx, cert_path) != 1) {
        zr_ssl_last_err(err_out);
        g_zr_ssl.SSL_CTX_free(ctx);
        return NULL;
    }
    if (g_zr_ssl.SSL_CTX_use_PrivateKey_file(ctx, key_path, ZR_SSL_FILETYPE_PEM) != 1) {
        zr_ssl_last_err(err_out);
        g_zr_ssl.SSL_CTX_free(ctx);
        return NULL;
    }
    if (g_zr_ssl.SSL_CTX_check_private_key && g_zr_ssl.SSL_CTX_check_private_key(ctx) != 1) {
        snprintf(err_out, 256, "TLS private key does not match certificate");
        g_zr_ssl.SSL_CTX_free(ctx);
        return NULL;
    }
    return ctx;
}

static bool zr_tls_client_wrap(ZrSock s, const char *host, ZrSslCtx **out_ctx, ZrSsl **out_ssl, char err_out[256]) {
    if (!zr_ensure_openssl(err_out)) return false;
    ZrSslCtx *ctx = g_zr_ssl.SSL_CTX_new(g_zr_ssl.TLS_client_method());
    if (!ctx) { zr_ssl_last_err(err_out); return false; }
    if (g_zr_ssl.SSL_CTX_ctrl) {
        g_zr_ssl.SSL_CTX_ctrl(ctx, ZR_SSL_CTRL_SET_MIN_PROTO_VER, ZR_TLS1_2_VERSION, NULL);
    }
    if (g_zr_ssl.SSL_CTX_set_default_verify_paths) {
        g_zr_ssl.SSL_CTX_set_default_verify_paths(ctx);
    }
    /* Support custom CA bundle via SSL_CERT_FILE or PBLANG_TLS_CA_FILE */
    const char *ca_file = getenv("PBLANG_TLS_CA_FILE");
    if (!ca_file || !ca_file[0]) ca_file = getenv("SSL_CERT_FILE");
    if (ca_file && ca_file[0] && g_zr_ssl.SSL_CTX_load_verify_locations) {
        g_zr_ssl.SSL_CTX_load_verify_locations(ctx, ca_file, NULL);
    } else if (!ca_file && zr_file_exists_reg("/etc/ssl/certs/ca-certificates.crt") && g_zr_ssl.SSL_CTX_load_verify_locations) {
        g_zr_ssl.SSL_CTX_load_verify_locations(ctx, "/etc/ssl/certs/ca-certificates.crt", NULL);
    }
    g_zr_ssl.SSL_CTX_set_verify(ctx, ZR_SSL_VERIFY_PEER, NULL);

    ZrSsl *ssl = g_zr_ssl.SSL_new(ctx);
    if (!ssl) { g_zr_ssl.SSL_CTX_free(ctx); zr_ssl_last_err(err_out); return false; }
    g_zr_ssl.SSL_set_fd(ssl, (int)s);
    if (g_zr_ssl.SSL_ctrl) {
        g_zr_ssl.SSL_ctrl(ssl, ZR_SSL_CTRL_SET_TLSEXT_HOSTNAME, 0, (void *)host);
    }
    if (g_zr_ssl.SSL_set1_host) {
        g_zr_ssl.SSL_set1_host(ssl, host);
    }
    if (g_zr_ssl.SSL_connect(ssl) != 1) {
        zr_ssl_last_err(err_out);
        g_zr_ssl.SSL_free(ssl);
        g_zr_ssl.SSL_CTX_free(ctx);
        return false;
    }
    if (g_zr_ssl.SSL_get_verify_result && g_zr_ssl.SSL_get_verify_result(ssl) != ZR_X509_V_OK) {
        snprintf(err_out, 256, "TLS X.509 certificate verification failed for '%s'", host);
        g_zr_ssl.SSL_free(ssl);
        g_zr_ssl.SSL_CTX_free(ctx);
        return false;
    }
    *out_ctx = ctx;
    *out_ssl = ssl;
    return true;
}

/* ---- TCP + TLS Builtins ---- */
static bool zr_net_arg_handle(PbVM *vm, PbValue v, const char *fn_name, size_t *out_id) {
    if (v.tag != VAL_NUM || !isfinite(v.u.num) || v.u.num < 1.0) {
        vm_raise(vm, "%s(): expected positive numeric handle", fn_name);
        return false;
    }
    *out_id = (size_t)v.u.num;
    return true;
}

static bool bi_tcp_listen(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_listen", "net")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "tcp_listen() expects 1 address string"); return false; }
    char err[256];
    ZrSock s = zr_bind_tcp_listener(((PbStr *)val_as_obj(argv[0]))->chars, err);
    if (s == ZR_INVALID_SOCK) { vm_raise(vm, "tcp_listen(): %s", err); return false; }
    size_t id = zr_net_insert(ZR_NET_LISTENER, s, NULL, NULL);
    if (!id) { zr_close_raw_sock(s); vm_raise(vm, "tcp_listen(): handle table full"); return false; }
    *out = val_num((double)id);
    return true;
}

static bool bi_tcp_connect(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_connect", "net")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "tcp_connect() expects 1 address string"); return false; }
    char host[512], port[32], err[256];
    if (!zr_split_host_port(((PbStr *)val_as_obj(argv[0]))->chars, host, port)) {
        vm_raise(vm, "tcp_connect(): expected 'host:port'");
        return false;
    }
    ZrSock s = zr_connect_tcp_raw(host, port, vm->caps.io_timeout_ms, err);
    if (s == ZR_INVALID_SOCK) { vm_raise(vm, "tcp_connect(): %s", err); return false; }
    size_t id = zr_net_insert(ZR_NET_STREAM, s, NULL, NULL);
    if (!id) { zr_close_raw_sock(s); vm_raise(vm, "tcp_connect(): handle table full"); return false; }
    *out = val_num((double)id);
    return true;
}

static bool bi_tcp_accept(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_accept", "net")) return false;
    if (argc != 1) { vm_raise(vm, "tcp_accept() expects 1 argument"); return false; }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_accept", &id)) return false;
    ZrNetEntry *le = zr_net_checkout(id);
    if (!le || (le->kind != ZR_NET_LISTENER && le->kind != ZR_NET_TLS_LISTENER)) {
        if (le) zr_net_checkin(le);
        vm_raise(vm, "tcp_accept(): invalid listener handle");
        return false;
    }

#if defined(__wasi__)
    zr_net_checkin(le);
    (void)out;
    vm_raise(vm, "tcp_accept(): unavailable in WASI sandbox");
    return false;
#else
    struct sockaddr_storage ss;
#if defined(_WIN32)
    int slen = (int)sizeof(ss);
#else
    socklen_t slen = (socklen_t)sizeof(ss);
#endif
    ZrSock cs = accept(le->sock, (struct sockaddr *)&ss, &slen);
    if (cs == ZR_INVALID_SOCK) {
        zr_net_checkin(le);
        vm_raise(vm, "tcp_accept(): accept() failed");
        return false;
    }
    zr_set_sock_timeouts(cs, vm->caps.io_timeout_ms);

    ZrSsl *ssl = NULL;
    if (le->kind == ZR_NET_TLS_LISTENER) {
        ssl = g_zr_ssl.SSL_new(le->ssl_ctx);
        if (!ssl) {
            zr_close_raw_sock(cs);
            zr_net_checkin(le);
            vm_raise(vm, "tcp_accept(): SSL_new failed");
            return false;
        }
        g_zr_ssl.SSL_set_fd(ssl, (int)cs);
        if (g_zr_ssl.SSL_accept(ssl) != 1) {
            char err[256];
            zr_ssl_last_err(err);
            g_zr_ssl.SSL_free(ssl);
            zr_close_raw_sock(cs);
            zr_net_checkin(le);
            vm_raise(vm, "tcp_accept() TLS: %s", err);
            return false;
        }
    }
    zr_net_checkin(le);

    size_t sid = zr_net_insert(ZR_NET_STREAM, cs, NULL, ssl);
    if (!sid) {
        if (ssl) g_zr_ssl.SSL_free(ssl);
        zr_close_raw_sock(cs);
        vm_raise(vm, "tcp_accept(): handle table full");
        return false;
    }
    *out = val_num((double)sid);
    return true;
#endif
}

static bool bi_tcp_read(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_read", "net")) return false;
    if (argc != 2 || argv[1].tag != VAL_NUM || argv[1].u.num < 0.0) {
        vm_raise(vm, "tcp_read(handle, max_bytes) expects (handle, non-negative number)");
        return false;
    }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_read", &id)) return false;
    size_t want = (size_t)argv[1].u.num;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_STREAM) {
        if (e) zr_net_checkin(e);
        vm_raise(vm, "tcp_read(): invalid stream handle");
        return false;
    }

    ZrBuf out_b;
    zr_buf_init(&out_b);
    if (e->rx_buf.len > 0) {
        size_t take = e->rx_buf.len < want ? e->rx_buf.len : want;
        zr_buf_append(&out_b, e->rx_buf.data, take);
        size_t rem = e->rx_buf.len - take;
        if (rem > 0) memmove(e->rx_buf.data, e->rx_buf.data + take, rem);
        e->rx_buf.len = rem;
    }

    char err[256];
    uint8_t tmp[4096];
    while (out_b.len < want) {
        size_t chunk = want - out_b.len;
        if (chunk > sizeof(tmp)) chunk = sizeof(tmp);
        ssize_t n = zr_stream_read_some(e, tmp, chunk, err);
        if (n < 0) {
            zr_net_checkin(e);
            zr_buf_free(&out_b);
            vm_raise(vm, "tcp_read(): %s", err);
            return false;
        }
        if (n == 0) break;
        zr_buf_append(&out_b, tmp, (size_t)n);
    }
    zr_net_checkin(e);
    *out = val_obj(vm_new_str(vm, (const char *)(out_b.data ? out_b.data : ""), out_b.len));
    zr_buf_free(&out_b);
    return true;
}

static ptrdiff_t zr_find_bytes(const uint8_t *hay, size_t hlen, const uint8_t *needle, size_t nlen) {
    if (nlen == 0) return 0;
    if (hlen < nlen) return -1;
    for (size_t i = 0; i <= hlen - nlen; i++) {
        if (memcmp(hay + i, needle, nlen) == 0) return (ptrdiff_t)i;
    }
    return -1;
}

static bool bi_tcp_read_until(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_read_until", "net")) return false;
    if ((argc != 2 && argc != 3) || !val_is_obj(argv[1], OBJ_STR) ||
        (argc == 3 && (argv[2].tag != VAL_NUM || argv[2].u.num < 0.0))) {
        vm_raise(vm, "tcp_read_until(handle, delim[, max_bytes]) expects (handle, string[, number])");
        return false;
    }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_read_until", &id)) return false;
    PbStr *delim = (PbStr *)val_as_obj(argv[1]);
    size_t max_bytes = (argc == 3) ? (size_t)argv[2].u.num : 65536u;

    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_STREAM) {
        if (e) zr_net_checkin(e);
        vm_raise(vm, "tcp_read_until(): invalid stream handle");
        return false;
    }

    size_t total_read = 0;
    char err[256];
    uint8_t tmp[4096];
    for (;;) {
        ptrdiff_t pos = zr_find_bytes((const uint8_t *)e->rx_buf.data, e->rx_buf.len, (const uint8_t *)delim->chars, delim->len);
        if (pos >= 0) {
            size_t end = (size_t)pos + delim->len;
            PbStr *res = vm_new_str(vm, (const char *)e->rx_buf.data, end);
            size_t rem = e->rx_buf.len - end;
            if (rem > 0) memmove(e->rx_buf.data, e->rx_buf.data + end, rem);
            e->rx_buf.len = rem;
            zr_net_checkin(e);
            *out = val_obj(res);
            return true;
        }
        if (total_read >= max_bytes) {
            PbStr *res = vm_new_str(vm, (const char *)(e->rx_buf.data ? e->rx_buf.data : ""), e->rx_buf.len);
            e->rx_buf.len = 0;
            zr_net_checkin(e);
            *out = val_obj(res);
            return true;
        }
        ssize_t n = zr_stream_read_some(e, tmp, sizeof(tmp), err);
        if (n < 0) {
            zr_net_checkin(e);
            vm_raise(vm, "tcp_read_until(): %s", err);
            return false;
        }
        if (n == 0) {
            PbStr *res = vm_new_str(vm, (const char *)(e->rx_buf.data ? e->rx_buf.data : ""), e->rx_buf.len);
            e->rx_buf.len = 0;
            zr_net_checkin(e);
            *out = val_obj(res);
            return true;
        }
        zr_buf_append(&e->rx_buf, tmp, (size_t)n);
        total_read += (size_t)n;
    }
}

static bool bi_tcp_write(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_write", "net")) return false;
    if (argc != 2) { vm_raise(vm, "tcp_write() expects 2 arguments"); return false; }
    if (!zr_check_taint_sink(vm, argv[1], "tcp_write")) return false;
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_write", &id)) return false;

    uint8_t *alloc_bytes = NULL;
    const uint8_t *ptr = NULL;
    size_t len = 0;
    if (val_is_obj(argv[1], OBJ_STR)) {
        PbStr *s = (PbStr *)val_as_obj(argv[1]);
        ptr = (const uint8_t *)s->chars;
        len = s->len;
    } else if (val_is_obj(argv[1], OBJ_BYTES)) {
        PbBytes *b = (PbBytes *)val_as_obj(argv[1]);
        ptr = b->data;
        len = b->len;
    } else if (val_is_obj(argv[1], OBJ_ARRAY)) {
        PbArray *a = (PbArray *)val_as_obj(argv[1]);
        alloc_bytes = (uint8_t *)zr_xmalloc(a->len > 0 ? a->len : 1);
        for (size_t i = 0; i < a->len; i++) {
            PbValue v = a->items[i];
            if (v.tag != VAL_NUM || !isfinite(v.u.num) || floor(v.u.num) != v.u.num || v.u.num < 0.0 || v.u.num > 255.0) {
                free(alloc_bytes);
                vm_raise(vm, "tcp_write(): byte array must contain numbers 0..255");
                return false;
            }
            alloc_bytes[i] = (uint8_t)v.u.num;
        }
        ptr = alloc_bytes;
        len = a->len;
    } else {
        vm_raise(vm, "tcp_write(): second argument must be string, bytes, or byte array");
        return false;
    }

    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_STREAM) {
        if (e) zr_net_checkin(e);
        free(alloc_bytes);
        vm_raise(vm, "tcp_write(): invalid stream handle");
        return false;
    }
    char err[256];
    ssize_t sent = zr_stream_write_all(e, ptr, len, err);
    zr_net_checkin(e);
    free(alloc_bytes);
    if (sent < 0) { vm_raise(vm, "tcp_write(): %s", err); return false; }
    *out = val_num((double)sent);
    return true;
}

static bool bi_tcp_close(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_close", "net")) return false;
    if (argc != 1) { vm_raise(vm, "tcp_close() expects 1 argument"); return false; }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_close", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (e) {
        zr_mutex_lock(&g_zr_net_mtx);
        zr_net_destroy_entry(e);
        zr_mutex_unlock(&g_zr_net_mtx);
    }
    *out = val_nil();
    return true;
}

static bool bi_tcp_local_addr(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_local_addr", "net")) return false;
    if (argc != 1) { vm_raise(vm, "tcp_local_addr() expects 1 argument"); return false; }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_local_addr", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e) { vm_raise(vm, "tcp_local_addr(): invalid handle"); return false; }
#if defined(__wasi__)
    zr_net_checkin(e);
    (void)out;
    vm_raise(vm, "tcp_local_addr(): unavailable in WASI sandbox");
    return false;
#else
    struct sockaddr_storage ss;
#if defined(_WIN32)
    int slen = (int)sizeof(ss);
#else
    socklen_t slen = (socklen_t)sizeof(ss);
#endif
    if (getsockname(e->sock, (struct sockaddr *)&ss, &slen) != 0) {
        zr_net_checkin(e);
        vm_raise(vm, "tcp_local_addr(): getsockname failed");
        return false;
    }
    zr_net_checkin(e);
#ifndef NI_MAXHOST
#define NI_MAXHOST 1025
#endif
#ifndef NI_MAXSERV
#define NI_MAXSERV 32
#endif
    char hbuf[NI_MAXHOST], sbuf[NI_MAXSERV];
    if (getnameinfo((struct sockaddr *)&ss, slen, hbuf, sizeof(hbuf), sbuf, sizeof(sbuf),
                    NI_NUMERICHOST | NI_NUMERICSERV) != 0) {
        vm_raise(vm, "tcp_local_addr(): getnameinfo failed");
        return false;
    }
    char full[NI_MAXHOST + NI_MAXSERV + 8];
    if (strchr(hbuf, ':')) snprintf(full, sizeof(full), "[%s]:%s", hbuf, sbuf);
    else snprintf(full, sizeof(full), "%s:%s", hbuf, sbuf);
    *out = val_obj(vm_new_cstr(vm, full));
    return true;
#endif
}

static bool bi_tcp_listen_tls(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_listen_tls", "net")) return false;
    if (argc != 3 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "tcp_listen_tls(addr, cert_path, key_path) expects 3 strings");
        return false;
    }
    char err[256];
    ZrSslCtx *ctx = zr_build_server_ssl_ctx(
        ((PbStr *)val_as_obj(argv[1]))->chars,
        ((PbStr *)val_as_obj(argv[2]))->chars,
        err);
    if (!ctx) { vm_raise(vm, "tcp_listen_tls(): %s", err); return false; }
    ZrSock s = zr_bind_tcp_listener(((PbStr *)val_as_obj(argv[0]))->chars, err);
    if (s == ZR_INVALID_SOCK) {
        g_zr_ssl.SSL_CTX_free(ctx);
        vm_raise(vm, "tcp_listen_tls(): %s", err);
        return false;
    }
    size_t id = zr_net_insert(ZR_NET_TLS_LISTENER, s, ctx, NULL);
    if (!id) {
        g_zr_ssl.SSL_CTX_free(ctx);
        zr_close_raw_sock(s);
        vm_raise(vm, "tcp_listen_tls(): handle table full");
        return false;
    }
    *out = val_num((double)id);
    return true;
}

static bool bi_tcp_connect_tls(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_connect_tls", "net")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || argv[1].tag != VAL_NUM ||
        argv[1].u.num < 0.0 || argv[1].u.num > 65535.0) {
        vm_raise(vm, "tcp_connect_tls(host, port) expects (string, port 0..65535)");
        return false;
    }
    const char *host = ((PbStr *)val_as_obj(argv[0]))->chars;
    char port_s[16], err[256];
    snprintf(port_s, sizeof(port_s), "%u", (unsigned)argv[1].u.num);
    ZrSock s = zr_connect_tcp_raw(host, port_s, vm->caps.io_timeout_ms, err);
    if (s == ZR_INVALID_SOCK) { vm_raise(vm, "tcp_connect_tls(): %s", err); return false; }

    ZrSslCtx *ctx = NULL;
    ZrSsl *ssl = NULL;
    if (!zr_tls_client_wrap(s, host, &ctx, &ssl, err)) {
        zr_close_raw_sock(s);
        vm_raise(vm, "tcp_connect_tls(): %s", err);
        return false;
    }
    size_t id = zr_net_insert(ZR_NET_STREAM, s, ctx, ssl);
    if (!id) {
        g_zr_ssl.SSL_free(ssl);
        g_zr_ssl.SSL_CTX_free(ctx);
        zr_close_raw_sock(s);
        vm_raise(vm, "tcp_connect_tls(): handle table full");
        return false;
    }
    *out = val_num((double)id);
    return true;
}

static bool bi_tcp_wrap_tls(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "tcp_wrap_tls", "net")) return false;
    if (argc != 3 || !val_is_obj(argv[1], OBJ_STR) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "tcp_wrap_tls(handle, cert_path, key_path) expects (handle, string, string)");
        return false;
    }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_wrap_tls", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_STREAM || e->ssl != NULL) {
        if (e) zr_net_checkin(e);
        vm_raise(vm, "tcp_wrap_tls(): handle must be a plain TCP stream");
        return false;
    }
    char err[256];
    ZrSslCtx *ctx = zr_build_server_ssl_ctx(
        ((PbStr *)val_as_obj(argv[1]))->chars,
        ((PbStr *)val_as_obj(argv[2]))->chars,
        err);
    if (!ctx) { zr_net_checkin(e); vm_raise(vm, "tcp_wrap_tls(): %s", err); return false; }
    ZrSsl *ssl = g_zr_ssl.SSL_new(ctx);
    if (!ssl) { g_zr_ssl.SSL_CTX_free(ctx); zr_net_checkin(e); vm_raise(vm, "tcp_wrap_tls(): SSL_new failed"); return false; }
    g_zr_ssl.SSL_set_fd(ssl, (int)e->sock);
    if (g_zr_ssl.SSL_accept(ssl) != 1) {
        zr_ssl_last_err(err);
        g_zr_ssl.SSL_free(ssl);
        g_zr_ssl.SSL_CTX_free(ctx);
        zr_net_checkin(e);
        vm_raise(vm, "tcp_wrap_tls(): %s", err);
        return false;
    }
    e->ssl_ctx = ctx;
    e->ssl = ssl;
    zr_net_checkin(e);
    *out = val_num((double)id);
    return true;
}

static bool bi_net_poll(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "net_poll", "net")) return false;
    if ((argc != 1 && argc != 2) || !val_is_obj(argv[0], OBJ_ARRAY) ||
        (argc == 2 && argv[1].tag != VAL_NUM)) {
        vm_raise(vm, "net_poll(handles_array[, timeout_ms]) expects (array[, number])");
        return false;
    }
    PbArray *in_arr = (PbArray *)val_as_obj(argv[0]);
    int timeout_ms = (argc == 2) ? (int)argv[1].u.num : 0;
    PbArray *ready = vm_new_array(vm, in_arr->len > 0 ? in_arr->len : 4);
    if (in_arr->len == 0) {
        *out = val_obj(ready);
        return true;
    }
#if defined(__wasi__)
    (void)timeout_ms;
    *out = val_obj(ready);
    return true;
#else
    vm_temp_push(vm, val_obj(ready));
    size_t n = in_arr->len;
    size_t *ids = (size_t *)zr_xcalloc(n, sizeof(size_t));
    ZrSock *socks = (ZrSock *)zr_xmalloc(n * sizeof(ZrSock));
    bool *pre_ready = (bool *)zr_xcalloc(n, sizeof(bool));
    bool any_pre_ready = false;

    for (size_t i = 0; i < n; i++) {
        socks[i] = ZR_INVALID_SOCK;
        if (!zr_net_arg_handle(vm, in_arr->items[i], "net_poll", &ids[i])) {
            free(ids); free(socks); free(pre_ready);
            vm_temp_pop(vm, 1);
            return false;
        }
    }

    zr_net_subsystem_init();
    zr_mutex_lock(&g_zr_net_mtx);
    for (size_t i = 0; i < n; i++) {
        for (size_t k = 0; k < ZR_MAX_NET_HANDLES; k++) {
            if (g_zr_net_table[k].kind != ZR_NET_NONE && g_zr_net_table[k].id == ids[i]) {
                socks[i] = g_zr_net_table[k].sock;
                if (g_zr_net_table[k].rx_buf.len > 0) {
                    pre_ready[i] = true;
                    any_pre_ready = true;
                }
                break;
            }
        }
    }
    zr_mutex_unlock(&g_zr_net_mtx);

    int eff_timeout = any_pre_ready ? 0 : timeout_ms;
#if defined(_WIN32)
    zr_ensure_winsock();
    fd_set rfds;
    FD_ZERO(&rfds);
    unsigned added = 0;
    for (size_t i = 0; i < n && added < FD_SETSIZE; i++) {
        if (socks[i] != ZR_INVALID_SOCK) {
            FD_SET(socks[i], &rfds);
            added++;
        }
    }
    if (added > 0) {
        struct timeval tv;
        struct timeval *tvp = NULL;
        if (eff_timeout >= 0) {
            tv.tv_sec = eff_timeout / 1000;
            tv.tv_usec = (eff_timeout % 1000) * 1000;
            tvp = &tv;
        }
        int sr = select(0, &rfds, NULL, NULL, tvp);
        if (sr > 0) {
            for (size_t i = 0; i < n; i++) {
                if (socks[i] != ZR_INVALID_SOCK && FD_ISSET(socks[i], &rfds)) {
                    pre_ready[i] = true;
                }
            }
        }
    }
#else
    struct pollfd *pfds = (struct pollfd *)zr_xcalloc(n, sizeof(struct pollfd));
    for (size_t i = 0; i < n; i++) {
        pfds[i].fd = (socks[i] >= 0) ? socks[i] : -1;
        pfds[i].events = POLLIN;
        pfds[i].revents = 0;
    }
    int pr = poll(pfds, (nfds_t)n, eff_timeout);
    if (pr > 0) {
        for (size_t i = 0; i < n; i++) {
            if (pfds[i].fd >= 0 && (pfds[i].revents & (POLLIN | POLLHUP | POLLERR))) {
                pre_ready[i] = true;
            }
        }
    }
    free(pfds);
#endif

    for (size_t i = 0; i < n; i++) {
        if (pre_ready[i]) {
            arr_push(ready, val_num((double)ids[i]));
        }
    }
    free(ids);
    free(socks);
    free(pre_ready);
    vm_temp_pop(vm, 1);
    *out = val_obj(ready);
    return true;
#endif
}

/* ---- Outbound HTTP/1.1 + HTTPS Client ---- */
static bool zr_parse_url(const char *url, bool *is_https, char host[512], uint16_t *port, char path[2048]) {
    const char *p = NULL;
    if (strncmp(url, "https://", 8) == 0) { *is_https = true; *port = 443; p = url + 8; }
    else if (strncmp(url, "http://", 7) == 0) { *is_https = false; *port = 80; p = url + 7; }
    else return false;

    const char *slash = strchr(p, '/');
    size_t auth_len = slash ? (size_t)(slash - p) : strlen(p);
    if (auth_len == 0 || auth_len >= 540) return false;
    char auth[544];
    memcpy(auth, p, auth_len);
    auth[auth_len] = '\0';
    snprintf(path, 2048, "%s", slash ? slash : "/");

    char port_s[32];
    if (zr_split_host_port(auth, host, port_s)) {
        long pv = strtol(port_s, NULL, 10);
        if (pv <= 0 || pv > 65535) return false;
        *port = (uint16_t)pv;
    } else {
        if (auth_len >= 512) return false;
        snprintf(host, 512, "%.510s", auth);
    }
    return host[0] != '\0';
}

static inline char zr_ascii_lc(char c) {
    return (c >= 'A' && c <= 'Z') ? (char)(c + ('a' - 'A')) : c;
}

static bool zr_ieq_str(const char *a, const char *b) {
    if (!a || !b) return false;
    while (*a && *b) {
        if (zr_ascii_lc(*a) != zr_ascii_lc(*b)) return false;
        a++;
        b++;
    }
    return *a == '\0' && *b == '\0';
}

static inline bool zr_istarts_with(const char *s, const char *pfx) {
    while (*pfx) {
        if (!*s || zr_ascii_lc(*s) != zr_ascii_lc(*pfx)) return false;
        s++;
        pfx++;
    }
    return true;
}

static bool zr_icontains_sub(const char *hay, const char *needle) {
    size_t nlen = strlen(needle);
    if (nlen == 0) return true;
    for (const char *p = hay; *p; p++) {
        size_t i = 0;
        while (i < nlen && p[i] && zr_ascii_lc(p[i]) == zr_ascii_lc(needle[i])) i++;
        if (i == nlen) return true;
    }
    return false;
}

/* ---- 8-Slot Thread-Safe HTTP/1.1 Keep-Alive Connection Pool ---- */
#define ZR_HTTP_POOL_CAP 8
typedef struct {
    bool      active;
    bool      is_https;
    uint16_t  port;
    char      host[512];
    ZrSock    sock;
    ZrSslCtx *ssl_ctx;
    ZrSsl    *ssl;
    uint64_t  last_tick;
} ZrHttpPoolSlot;

static ZrHttpPoolSlot g_zr_http_pool[ZR_HTTP_POOL_CAP];
static uint64_t       g_zr_http_pool_tick = 0;

static bool zr_sock_idle_healthy(ZrSock s) {
    if (s == ZR_INVALID_SOCK) return false;
#if defined(__wasi__)
    return false;
#elif defined(_WIN32)
    fd_set rfds, efds;
    FD_ZERO(&rfds);
    FD_ZERO(&efds);
    FD_SET(s, &rfds);
    FD_SET(s, &efds);
    struct timeval tv;
    tv.tv_sec = 0;
    tv.tv_usec = 0;
    /* Any readable/error readiness on an idle keep-alive client socket means peer FIN or stale unread data */
    if (select(0, &rfds, NULL, &efds, &tv) != 0) return false;
    return true;
#else
    struct pollfd pfd;
    pfd.fd = s;
    pfd.events = POLLIN;
    pfd.revents = 0;
    if (poll(&pfd, 1, 0) != 0) return false;
    return true;
#endif
}

static void zr_http_pool_close_slot_unlocked(ZrHttpPoolSlot *sl) {
    if (!sl->active) return;
    ZrNetEntry tmp;
    memset(&tmp, 0, sizeof(tmp));
    tmp.kind = ZR_NET_STREAM;
    tmp.sock = sl->sock;
    tmp.ssl_ctx = sl->ssl_ctx;
    tmp.ssl = sl->ssl;
    zr_net_destroy_entry(&tmp);
    memset(sl, 0, sizeof(*sl));
    sl->sock = ZR_INVALID_SOCK;
}

static bool zr_http_pool_acquire(const char *host, uint16_t port, bool is_https, uint64_t timeout_ms, ZrNetEntry *out_e) {
    zr_net_subsystem_init();
    zr_mutex_lock(&g_zr_net_mtx);
    for (size_t i = 0; i < ZR_HTTP_POOL_CAP; i++) {
        ZrHttpPoolSlot *sl = &g_zr_http_pool[i];
        if (sl->active && sl->is_https == is_https && sl->port == port && zr_ieq_str(sl->host, host)) {
            if (!zr_sock_idle_healthy(sl->sock)) {
                zr_http_pool_close_slot_unlocked(sl);
                continue;
            }
            memset(out_e, 0, sizeof(*out_e));
            out_e->kind = ZR_NET_STREAM;
            out_e->sock = sl->sock;
            out_e->ssl_ctx = sl->ssl_ctx;
            out_e->ssl = sl->ssl;
            zr_set_sock_timeouts(out_e->sock, timeout_ms ? timeout_ms : 15000ULL);
            memset(sl, 0, sizeof(*sl));
            sl->sock = ZR_INVALID_SOCK;
            zr_mutex_unlock(&g_zr_net_mtx);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_net_mtx);
    return false;
}

static void zr_http_pool_release(const char *host, uint16_t port, bool is_https, ZrNetEntry *e) {
    if (!e || e->sock == ZR_INVALID_SOCK) return;
    zr_net_subsystem_init();
    zr_mutex_lock(&g_zr_net_mtx);
    if (!zr_sock_idle_healthy(e->sock)) {
        zr_mutex_unlock(&g_zr_net_mtx);
        zr_net_destroy_entry(e);
        return;
    }
    size_t target = 0;
    bool found_free = false;
    uint64_t oldest_tick = UINT64_MAX;
    for (size_t i = 0; i < ZR_HTTP_POOL_CAP; i++) {
        if (!g_zr_http_pool[i].active) {
            target = i;
            found_free = true;
            break;
        }
        if (g_zr_http_pool[i].last_tick < oldest_tick) {
            oldest_tick = g_zr_http_pool[i].last_tick;
            target = i;
        }
    }
    if (!found_free) {
        zr_http_pool_close_slot_unlocked(&g_zr_http_pool[target]);
    }
    ZrHttpPoolSlot *sl = &g_zr_http_pool[target];
    sl->active = true;
    sl->is_https = is_https;
    sl->port = port;
    snprintf(sl->host, sizeof(sl->host), "%.510s", host);
    sl->sock = e->sock;
    sl->ssl_ctx = e->ssl_ctx;
    sl->ssl = e->ssl;
    sl->last_tick = ++g_zr_http_pool_tick;
    zr_buf_free(&e->rx_buf);
    memset(e, 0, sizeof(*e));
    e->sock = ZR_INVALID_SOCK;
    zr_mutex_unlock(&g_zr_net_mtx);
}

static inline bool zr_decode_chunked_body(const uint8_t *in, size_t in_len, ZrBuf *out) {
    size_t pos = 0;
    while (pos < in_len) {
        size_t line_end = pos;
        while (line_end + 1 < in_len && !(in[line_end] == '\r' && in[line_end + 1] == '\n')) line_end++;
        if (line_end + 1 >= in_len) break;
        size_t chunk_sz = 0;
        for (size_t i = pos; i < line_end; i++) {
            if (in[i] == ';' || in[i] == ' ' || in[i] == '\t') break;
            int nib = zr_hex_nibble((char)in[i]);
            if (nib < 0) break;
            chunk_sz = (chunk_sz << 4) | (size_t)nib;
        }
        pos = line_end + 2;
        if (chunk_sz == 0) break;
        if (pos + chunk_sz > in_len) {
            zr_buf_append(out, in + pos, in_len - pos);
            break;
        }
        zr_buf_append(out, in + pos, chunk_sz);
        pos += chunk_sz;
        if (pos + 2 <= in_len && in[pos] == '\r' && in[pos + 1] == '\n') pos += 2;
    }
    return true;
}

typedef struct {
    PbValue  on_chunk;
    bool     has_cb;
    int      status_code;
    PbMap   *headers_map;
    size_t   bytes_received;
    size_t   chunks_delivered;
    bool     stopped_by_cb;
} ZrHttpStreamSink;

static bool zr_http_deliver_slice(PbVM *vm, ZrHttpStreamSink *sink, ZrBuf *resp_body,
                                  const uint8_t *data, size_t len) {
    if (len == 0) return true;
    sink->bytes_received += len;
    if (sink->has_cb) {
        PbStr *chunk_s = vm_new_str(vm, (const char *)data, len);
        vm_temp_push(vm, val_obj(chunk_s));
        int cb_argc = 2;
        if (sink->on_chunk.tag == VAL_OBJ && sink->on_chunk.as.obj->type == OBJ_CLOSURE) {
            PbClosure *cl = (PbClosure *)sink->on_chunk.as.obj;
            if (!cl->fn->is_vararg && cl->fn->arity == 1) cb_argc = 1;
        }
        PbValue args[2];
        args[0] = val_obj(chunk_s);
        args[1] = val_nil();
        if (cb_argc == 2) {
            PbMap *meta = vm_new_map(vm, 4);
            vm_temp_push(vm, val_obj(meta));
            map_set(meta, vm_const_str(vm, "status"), val_num((double)sink->status_code));
            if (sink->headers_map) map_set(meta, vm_const_str(vm, "headers"), val_obj(sink->headers_map));
            map_set(meta, vm_const_str(vm, "bytes_so_far"), val_num((double)sink->bytes_received));
            map_set(meta, vm_const_str(vm, "chunk_index"), val_num((double)sink->chunks_delivered));
            args[1] = val_obj(meta);
        }
        sink->chunks_delivered++;
        PbValue ret = val_nil();
        bool ok = vm_call_value_sync(vm, sink->on_chunk, cb_argc, args, &ret);
        vm_temp_pop(vm, (cb_argc == 2) ? 2 : 1);
        if (!ok) return false;
        if (ret.tag == VAL_BOOL && !ret.as.b) {
            sink->stopped_by_cb = true;
            return true;
        }
        return true;
    }
    if (resp_body) zr_buf_append(resp_body, data, len);
    return true;
}

static ptrdiff_t zr_find_hdr_end(const uint8_t *buf, size_t len, size_t *out_sep_len) {
    for (size_t i = 0; i + 1 < len; i++) {
        if (buf[i] == '\r' && i + 3 < len && buf[i + 1] == '\n' && buf[i + 2] == '\r' && buf[i + 3] == '\n') {
            *out_sep_len = 4;
            return (ptrdiff_t)i;
        }
        if (buf[i] == '\n' && buf[i + 1] == '\n') {
            *out_sep_len = 2;
            return (ptrdiff_t)i;
        }
    }
    return -1;
}

static void zr_parse_http_headers_into(PbVM *vm, const char *hdr_str, PbMap *hdr_map,
                                       bool *is_http10, bool *conn_close, bool *conn_keep_alive,
                                       bool *is_chunked, bool *has_cl, uint64_t *content_length,
                                       char loc_out[2048]) {
    *is_http10 = (strncmp(hdr_str, "HTTP/1.0", 8) == 0);
    *conn_close = false;
    *conn_keep_alive = false;
    *is_chunked = false;
    *has_cl = false;
    *content_length = 0;
    if (loc_out) loc_out[0] = '\0';

    const char *line = strchr(hdr_str, '\n');
    if (!line) return;
    line++;
    while (*line) {
        const char *eol = strchr(line, '\n');
        size_t llen = eol ? (size_t)(eol - line) : strlen(line);
        if (llen > 0 && line[llen - 1] == '\r') llen--;
        const char *colon = (const char *)memchr(line, ':', llen);
        if (colon && colon > line) {
            size_t klen = (size_t)(colon - line);
            while (klen > 0 && (line[klen - 1] == ' ' || line[klen - 1] == '\t')) klen--;
            const char *vp = colon + 1;
            size_t vlen = llen - (size_t)(vp - line);
            while (vlen > 0 && (*vp == ' ' || *vp == '\t')) { vp++; vlen--; }
            while (vlen > 0 && (vp[vlen - 1] == ' ' || vp[vlen - 1] == '\t')) vlen--;

            char klow[256];
            size_t kl = klen < sizeof(klow) - 1 ? klen : sizeof(klow) - 1;
            for (size_t i = 0; i < kl; i++) klow[i] = zr_ascii_lc(line[i]);
            klow[kl] = '\0';

            char *vcopy = (char *)zr_xmalloc(vlen + 1);
            memcpy(vcopy, vp, vlen);
            vcopy[vlen] = '\0';

            if (hdr_map) {
                PbStr *ks = vm_new_str(vm, klow, kl);
                vm_temp_push(vm, val_obj(ks));
                PbStr *vs = vm_new_str(vm, vcopy, vlen);
                map_set(hdr_map, ks, val_obj(vs));
                vm_temp_pop(vm, 1);
            }
            if (strcmp(klow, "connection") == 0) {
                if (zr_icontains_sub(vcopy, "close")) *conn_close = true;
                if (zr_icontains_sub(vcopy, "keep-alive")) *conn_keep_alive = true;
            } else if (strcmp(klow, "transfer-encoding") == 0) {
                if (zr_icontains_sub(vcopy, "chunked")) *is_chunked = true;
            } else if (strcmp(klow, "content-length") == 0) {
                char *endp = NULL;
                unsigned long long clv = strtoull(vcopy, &endp, 10);
                if (endp && endp != vcopy) {
                    *has_cl = true;
                    *content_length = (uint64_t)clv;
                }
            } else if (loc_out && strcmp(klow, "location") == 0) {
                snprintf(loc_out, 2048, "%s", vcopy);
            }
            free(vcopy);
        }
        if (!eol) break;
        line = eol + 1;
    }
    if (*is_chunked) *has_cl = false;
}

static bool zr_http_read_response_on_entry(PbVM *vm, ZrNetEntry *e, const char *method, const char *url,
                                           const char *host, uint16_t port, bool is_https,
                                           bool caller_wants_close, ZrBuf *resp_body,
                                           ZrHttpStreamSink *sink, PbMap *hdr_map,
                                           int *out_status, char redirect_loc[2048],
                                           bool *out_zero_bytes_eof, char err_out[256]) {
    *out_zero_bytes_eof = false;
    if (redirect_loc) redirect_loc[0] = '\0';

    ZrBuf raw;
    zr_buf_init(&raw);
    uint8_t rbuf[16384];
    int status_code = 0;
    size_t body_offset = 0;
    bool is_http10 = false, conn_close = false, conn_keep_alive = false, is_chunked = false, has_cl = false;
    uint64_t content_length = 0;

    for (;;) {
        size_t sep_len = 0;
        ptrdiff_t sep = raw.data ? zr_find_hdr_end((const uint8_t *)raw.data, raw.len, &sep_len) : -1;
        if (sep >= 0) {
            char *hdr_str = (char *)zr_xmalloc((size_t)sep + 1);
            memcpy(hdr_str, raw.data, (size_t)sep);
            hdr_str[sep] = '\0';
            status_code = 0;
            sscanf(hdr_str, "HTTP/%*s %d", &status_code);
            if (status_code >= 100 && status_code < 200 && status_code != 101) {
                free(hdr_str);
                size_t drop = (size_t)sep + sep_len;
                if (raw.len > drop) memmove(raw.data, raw.data + drop, raw.len - drop);
                raw.len -= drop;
                continue;
            }
            zr_parse_http_headers_into(vm, hdr_str, hdr_map, &is_http10, &conn_close, &conn_keep_alive,
                                       &is_chunked, &has_cl, &content_length, redirect_loc);
            free(hdr_str);
            body_offset = (size_t)sep + sep_len;
            break;
        }
        if (raw.len > 65536) {
            zr_buf_free(&raw);
            snprintf(err_out, 256, "HTTP response headers exceed 64 KiB for '%s'", url);
            return false;
        }
        ssize_t n = zr_stream_read_some(e, rbuf, sizeof(rbuf), err_out);
        if (n <= 0) {
            if (raw.len == 0) *out_zero_bytes_eof = true;
            zr_buf_free(&raw);
            if (err_out[0] == '\0') snprintf(err_out, 256, "unexpected EOF reading HTTP headers from '%s'", url);
            return false;
        }
        zr_buf_append(&raw, rbuf, (size_t)n);
    }

    if (out_status) *out_status = status_code;
    sink->status_code = status_code;

    /* Bodyless responses per RFC 9112 */
    bool no_body = (zr_ieq_str(method, "HEAD") || status_code == 204 || status_code == 304 ||
                    (status_code >= 100 && status_code < 200));
    bool framing_clean = false;

    if (no_body) {
        framing_clean = (raw.len == body_offset);
    } else if (is_chunked) {
        ZrBuf cbuf;
        zr_buf_init(&cbuf);
        if (raw.len > body_offset) {
            zr_buf_append(&cbuf, (const uint8_t *)raw.data + body_offset, raw.len - body_offset);
        }
        size_t cpos = 0;
        bool done_chunks = false;
        while (!done_chunks && !sink->stopped_by_cb) {
            /* Compact consumed prefix periodically */
            if (cpos > 32768) {
                if (cbuf.len > cpos) memmove(cbuf.data, cbuf.data + cpos, cbuf.len - cpos);
                cbuf.len -= cpos;
                cpos = 0;
            }
            /* Find \r\n terminating chunk-size line */
            size_t line_end = cpos;
            while (line_end + 1 < cbuf.len && !(cbuf.data[line_end] == '\r' && cbuf.data[line_end + 1] == '\n')) line_end++;
            if (line_end + 1 >= cbuf.len) {
                ssize_t n = zr_stream_read_some(e, rbuf, sizeof(rbuf), err_out);
                if (n <= 0) break;
                zr_buf_append(&cbuf, rbuf, (size_t)n);
                continue;
            }
            size_t chunk_sz = 0;
            bool hex_ok = false;
            for (size_t i = cpos; i < line_end; i++) {
                char ch = cbuf.data[i];
                if (ch == ';' || ch == ' ' || ch == '\t') break;
                int nib = zr_hex_nibble(ch);
                if (nib < 0) { hex_ok = false; break; }
                hex_ok = true;
                chunk_sz = (chunk_sz << 4) | (size_t)nib;
            }
            if (!hex_ok) break;
            cpos = line_end + 2;

            if (chunk_sz == 0) {
                /* Consume optional trailer headers + final \r\n */
                for (;;) {
                    if (cbuf.len - cpos >= 2 && cbuf.data[cpos] == '\r' && cbuf.data[cpos + 1] == '\n') {
                        cpos += 2;
                        framing_clean = (cpos == cbuf.len);
                        done_chunks = true;
                        break;
                    }
                    ptrdiff_t tr_end = (cbuf.len > cpos)
                        ? zr_find_bytes((const uint8_t *)cbuf.data + cpos, cbuf.len - cpos, (const uint8_t *)"\r\n\r\n", 4)
                        : -1;
                    if (tr_end >= 0) {
                        cpos += (size_t)tr_end + 4;
                        framing_clean = (cpos == cbuf.len);
                        done_chunks = true;
                        break;
                    }
                    ssize_t n = zr_stream_read_some(e, rbuf, sizeof(rbuf), err_out);
                    if (n <= 0) { done_chunks = true; break; }
                    zr_buf_append(&cbuf, rbuf, (size_t)n);
                }
                break;
            }

            /* Stream/buffer chunk_sz bytes incrementally */
            size_t rem = chunk_sz;
            while (rem > 0 && !sink->stopped_by_cb) {
                size_t avail = cbuf.len - cpos;
                if (avail > 0) {
                    size_t take = avail < rem ? avail : rem;
                    if (!zr_http_deliver_slice(vm, sink, resp_body, (const uint8_t *)cbuf.data + cpos, take)) {
                        zr_buf_free(&cbuf);
                        zr_buf_free(&raw);
                        return false;
                    }
                    cpos += take;
                    rem -= take;
                } else {
                    size_t to_read = rem < sizeof(rbuf) ? rem : sizeof(rbuf);
                    ssize_t n = zr_stream_read_some(e, rbuf, to_read, err_out);
                    if (n <= 0) { rem = 0; done_chunks = true; break; }
                    if (!zr_http_deliver_slice(vm, sink, resp_body, rbuf, (size_t)n)) {
                        zr_buf_free(&cbuf);
                        zr_buf_free(&raw);
                        return false;
                    }
                    rem -= (size_t)n;
                }
            }
            if (done_chunks || sink->stopped_by_cb) break;

            /* Consume trailing \r\n after chunk data */
            while (cbuf.len - cpos < 2) {
                ssize_t n = zr_stream_read_some(e, rbuf, sizeof(rbuf), err_out);
                if (n <= 0) { done_chunks = true; break; }
                zr_buf_append(&cbuf, rbuf, (size_t)n);
            }
            if (cbuf.len - cpos >= 2 && cbuf.data[cpos] == '\r' && cbuf.data[cpos + 1] == '\n') {
                cpos += 2;
            } else {
                break;
            }
        }
        zr_buf_free(&cbuf);
    } else if (has_cl) {
        uint64_t rem = content_length;
        size_t init_avail = (raw.len > body_offset) ? (raw.len - body_offset) : 0;
        if (init_avail > 0 && rem > 0) {
            size_t take = ((uint64_t)init_avail < rem) ? init_avail : (size_t)rem;
            if (!zr_http_deliver_slice(vm, sink, resp_body, (const uint8_t *)raw.data + body_offset, take)) {
                zr_buf_free(&raw);
                return false;
            }
            rem -= (uint64_t)take;
        }
        while (rem > 0 && !sink->stopped_by_cb) {
            size_t want = (rem < (uint64_t)sizeof(rbuf)) ? (size_t)rem : sizeof(rbuf);
            ssize_t n = zr_stream_read_some(e, rbuf, want, err_out);
            if (n <= 0) break;
            if (!zr_http_deliver_slice(vm, sink, resp_body, rbuf, (size_t)n)) {
                zr_buf_free(&raw);
                return false;
            }
            rem -= (uint64_t)n;
        }
        if (rem == 0 && !sink->stopped_by_cb && init_avail <= content_length) {
            framing_clean = true;
        }
    } else {
        /* Close-delimited response body */
        if (raw.len > body_offset) {
            if (!zr_http_deliver_slice(vm, sink, resp_body, (const uint8_t *)raw.data + body_offset, raw.len - body_offset)) {
                zr_buf_free(&raw);
                return false;
            }
        }
        while (!sink->stopped_by_cb) {
            ssize_t n = zr_stream_read_some(e, rbuf, sizeof(rbuf), err_out);
            if (n <= 0) break;
            if (!zr_http_deliver_slice(vm, sink, resp_body, rbuf, (size_t)n)) {
                zr_buf_free(&raw);
                return false;
            }
            if (!sink->has_cb && resp_body && resp_body->len > 64 * 1024 * 1024) break;
        }
        framing_clean = false;
    }

    zr_buf_free(&raw);

    bool keep_open = framing_clean && !sink->stopped_by_cb && !caller_wants_close &&
                     !conn_close && (!is_http10 || conn_keep_alive);
    if (keep_open) {
        zr_http_pool_release(host, port, is_https, e);
    } else {
        zr_net_destroy_entry(e);
    }
    return true;
}

static bool zr_http_exchange(PbVM *vm, const char *method, const char *url,
                             const char **hdr_keys, const char **hdr_vals, size_t nhdrs,
                             const uint8_t *body, size_t body_len, ZrBuf *resp_body,
                             PbMap *out_headers, ZrHttpStreamSink *opt_sink,
                             int redirect_depth, bool raise_on_http_error, int *out_status) {
    if (out_status) *out_status = 0;
    if (redirect_depth > 5) {
        vm_raise(vm, "http_%s(): too many redirects", method);
        return false;
    }
    bool is_https = false;
    char host[512], path[2048];
    uint16_t port = 80;
    if (!zr_parse_url(url, &is_https, host, &port, path)) {
        vm_raise(vm, "http request: invalid URL '%s'", url);
        return false;
    }

    ZrHttpStreamSink default_sink;
    memset(&default_sink, 0, sizeof(default_sink));
    default_sink.on_chunk = val_nil();
    default_sink.headers_map = out_headers;
    ZrHttpStreamSink *sink = opt_sink ? opt_sink : &default_sink;
    sink->headers_map = out_headers;

#if defined(_WIN32)
    /* On Windows, use WinHTTP for HTTPS when OpenSSL DLL is not present so OS root store works natively;
       for HTTP (and HTTPS with OpenSSL), use the fast pooled HTTP/1.1 Keep-Alive engine! */
    char ssl_probe[256];
    ssl_probe[0] = '\0';
    if (is_https && !zr_ensure_openssl(ssl_probe)) {
        wchar_t whost[512], wpath[2048], wmethod[16];
        MultiByteToWideChar(CP_UTF8, 0, host, -1, whost, 512);
        MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, 2048);
        MultiByteToWideChar(CP_UTF8, 0, method, -1, wmethod, 16);

        HINTERNET hSession = WinHttpOpen(L"pblang-zr/4.2", WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                         WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
        if (!hSession) { vm_raise(vm, "WinHttpOpen failed"); return false; }
        int tms = (int)(vm->caps.io_timeout_ms ? vm->caps.io_timeout_ms : 30000);
        WinHttpSetTimeouts(hSession, tms, tms, tms, tms);

        HINTERNET hConnect = WinHttpConnect(hSession, whost, (INTERNET_PORT)port, 0);
        if (!hConnect) { WinHttpCloseHandle(hSession); vm_raise(vm, "WinHttpConnect failed"); return false; }

        HINTERNET hReq = WinHttpOpenRequest(hConnect, wmethod, wpath, NULL, WINHTTP_NO_REFERER,
                                            WINHTTP_DEFAULT_ACCEPT_TYPES, WINHTTP_FLAG_SECURE);
        if (!hReq) { WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession); vm_raise(vm, "WinHttpOpenRequest failed"); return false; }

        for (size_t i = 0; i < nhdrs; i++) {
            char hline[2048];
            snprintf(hline, sizeof(hline), "%s: %s", hdr_keys[i], hdr_vals[i]);
            wchar_t whline[2048];
            MultiByteToWideChar(CP_UTF8, 0, hline, -1, whline, 2048);
            WinHttpAddRequestHeaders(hReq, whline, (DWORD)-1L, WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE);
        }

        BOOL ok = WinHttpSendRequest(hReq, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
                                     (LPVOID)body, (DWORD)body_len, (DWORD)body_len, 0);
        if (ok) ok = WinHttpReceiveResponse(hReq, NULL);
        if (!ok) {
            WinHttpCloseHandle(hReq); WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession);
            vm_raise(vm, "WinHTTP request failed for '%s'", url);
            return false;
        }
        DWORD wstatus = 200;
        DWORD wstatus_sz = sizeof(wstatus);
        WinHttpQueryHeaders(hReq, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX, &wstatus, &wstatus_sz, WINHTTP_NO_HEADER_INDEX);
        if (out_status) *out_status = (int)wstatus;
        sink->status_code = (int)wstatus;
        if (out_headers) {
            DWORD rh_sz = 0;
            WinHttpQueryHeaders(hReq, WINHTTP_QUERY_RAW_HEADERS_CRLF, WINHTTP_HEADER_NAME_BY_INDEX,
                                NULL, &rh_sz, WINHTTP_NO_HEADER_INDEX);
            if (rh_sz > 0 && rh_sz < 131072) {
                wchar_t *whdrs = (wchar_t *)zr_xcalloc(rh_sz + 4, 1);
                if (WinHttpQueryHeaders(hReq, WINHTTP_QUERY_RAW_HEADERS_CRLF, WINHTTP_HEADER_NAME_BY_INDEX,
                                        whdrs, &rh_sz, WINHTTP_NO_HEADER_INDEX)) {
                    int u8len = WideCharToMultiByte(CP_UTF8, 0, whdrs, -1, NULL, 0, NULL, NULL);
                    if (u8len > 0) {
                        char *u8h = (char *)zr_xmalloc((size_t)u8len + 1);
                        WideCharToMultiByte(CP_UTF8, 0, whdrs, -1, u8h, u8len, NULL, NULL);
                        u8h[u8len] = '\0';
                        bool d1, d2, d3, d4, d5;
                        uint64_t dcl;
                        zr_parse_http_headers_into(vm, u8h, out_headers, &d1, &d2, &d3, &d4, &d5, &dcl, NULL);
                        free(u8h);
                    }
                }
                free(whdrs);
            }
        }
        if (raise_on_http_error && wstatus >= 400) {
            WinHttpCloseHandle(hReq); WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession);
            vm_raise(vm, "http status %d for '%s'", (int)wstatus, url);
            return false;
        }
        DWORD avail = 0;
        while (!sink->stopped_by_cb && WinHttpQueryDataAvailable(hReq, &avail) && avail > 0) {
            uint8_t *tmp = (uint8_t *)zr_xmalloc(avail);
            DWORD got = 0;
            bool step_ok = true;
            if (WinHttpReadData(hReq, tmp, avail, &got) && got > 0) {
                step_ok = zr_http_deliver_slice(vm, sink, resp_body, tmp, (size_t)got);
            }
            free(tmp);
            if (!step_ok) {
                WinHttpCloseHandle(hReq); WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession);
                return false;
            }
        }
        WinHttpCloseHandle(hReq);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return true;
    }
#endif

    bool has_ua = false, has_conn = false, has_cl_hdr = false, caller_wants_close = false;
    for (size_t i = 0; i < nhdrs; i++) {
        if (!hdr_keys[i]) continue;
        if (zr_ieq_str(hdr_keys[i], "User-Agent")) has_ua = true;
        if (zr_ieq_str(hdr_keys[i], "Content-Length")) has_cl_hdr = true;
        if (zr_ieq_str(hdr_keys[i], "Connection")) {
            has_conn = true;
            if (hdr_vals[i] && zr_icontains_sub(hdr_vals[i], "close")) caller_wants_close = true;
        }
    }

    ZrBuf req;
    zr_buf_init(&req);
    char head_top[3072];
    const char *def_ua = has_ua ? "" : "User-Agent: pblang/4.2\r\n";
    const char *def_conn = has_conn ? "" : "Connection: keep-alive\r\n";
    if ((is_https && port == 443) || (!is_https && port == 80)) {
        snprintf(head_top, sizeof(head_top), "%s %s HTTP/1.1\r\nHost: %s\r\n%s%s",
                 method, path, host, def_ua, def_conn);
    } else {
        snprintf(head_top, sizeof(head_top), "%s %s HTTP/1.1\r\nHost: %s:%u\r\n%s%s",
                 method, path, host, (unsigned)port, def_ua, def_conn);
    }
    zr_buf_append_cstr(&req, head_top);
    for (size_t i = 0; i < nhdrs; i++) {
        if (!hdr_keys[i] || !hdr_vals[i]) continue;
        zr_buf_append_cstr(&req, hdr_keys[i]);
        zr_buf_append_cstr(&req, ": ");
        zr_buf_append_cstr(&req, hdr_vals[i]);
        zr_buf_append_cstr(&req, "\r\n");
    }
    if (!has_cl_hdr && (body_len > 0 || zr_ieq_str(method, "POST") || zr_ieq_str(method, "PUT") || zr_ieq_str(method, "PATCH"))) {
        char cl[64];
        snprintf(cl, sizeof(cl), "Content-Length: %zu\r\n", body_len);
        zr_buf_append_cstr(&req, cl);
    }
    zr_buf_append_cstr(&req, "\r\n");
    if (body_len > 0) zr_buf_append(&req, body, body_len);

    char port_s[16], err[256];
    snprintf(port_s, sizeof(port_s), "%u", (unsigned)port);
    err[0] = '\0';

    ZrNetEntry tmp_e;
    memset(&tmp_e, 0, sizeof(tmp_e));
    tmp_e.sock = ZR_INVALID_SOCK;
    bool reused = zr_http_pool_acquire(host, port, is_https, vm->caps.io_timeout_ms, &tmp_e);

    for (int attempt = 0; attempt < 2; attempt++) {
        err[0] = '\0';
        if (!reused) {
            ZrSock s = zr_connect_tcp_raw(host, port_s, vm->caps.io_timeout_ms, err);
            if (s == ZR_INVALID_SOCK) {
                zr_buf_free(&req);
                vm_raise(vm, "http %s '%s': %s", method, url, err);
                return false;
            }
            memset(&tmp_e, 0, sizeof(tmp_e));
            tmp_e.kind = ZR_NET_STREAM;
            tmp_e.sock = s;
            if (is_https) {
                if (!zr_tls_client_wrap(s, host, &tmp_e.ssl_ctx, &tmp_e.ssl, err)) {
                    zr_close_raw_sock(s);
                    zr_buf_free(&req);
                    vm_raise(vm, "https %s '%s': %s", method, url, err);
                    return false;
                }
            }
        }

        if (zr_stream_write_all(&tmp_e, (const uint8_t *)req.data, req.len, err) < 0) {
            zr_net_destroy_entry(&tmp_e);
            if (reused && attempt == 0) { reused = false; continue; }
            zr_buf_free(&req);
            vm_raise(vm, "http write '%s': %s", url, err);
            return false;
        }

        int status_code = 0;
        char redirect_loc[2048];
        redirect_loc[0] = '\0';
        bool zero_eof = false;
        ZrBuf step_body;
        zr_buf_init(&step_body);

        /* Hold back streaming callback if we might follow a 3xx redirect; wait: zr_http_read_response_on_entry
           only streams body bytes — for 3xx redirects we buffer into step_body and recurse! */
        bool pass_ok = zr_http_read_response_on_entry(
            vm, &tmp_e, method, url, host, port, is_https, caller_wants_close,
            sink->has_cb ? NULL : &step_body, sink, out_headers,
            &status_code, redirect_loc, &zero_eof, err);

        if (!pass_ok) {
            zr_buf_free(&step_body);
            zr_net_destroy_entry(&tmp_e);
            if (reused && zero_eof && attempt == 0 && sink->chunks_delivered == 0) {
                reused = false;
                continue;
            }
            zr_buf_free(&req);
            if (vm->err_msg[0] == '\0') vm_raise(vm, "http '%s': %s", url, err[0] ? err : "read failed");
            return false;
        }

        zr_buf_free(&req);
        if (out_status) *out_status = status_code;

        if (!sink->has_cb && redirect_loc[0] != '\0' &&
            (status_code == 301 || status_code == 302 || status_code == 303 || status_code == 307 || status_code == 308)) {
            char next_url[2560];
            if (strncmp(redirect_loc, "http://", 7) == 0 || strncmp(redirect_loc, "https://", 8) == 0) {
                snprintf(next_url, sizeof(next_url), "%s", redirect_loc);
            } else {
                snprintf(next_url, sizeof(next_url), "%s://%s:%u%s%s",
                         is_https ? "https" : "http", host, (unsigned)port,
                         redirect_loc[0] == '/' ? "" : "/", redirect_loc);
            }
            zr_buf_free(&step_body);
            if (out_headers) {
                for (size_t hi = 0; hi < out_headers->used; hi++) out_headers->entries[hi].active = false;
                out_headers->used = 0;
                out_headers->len = 0;
                if (out_headers->buckets) {
                    for (size_t bi = 0; bi < out_headers->bcap; bi++) out_headers->buckets[bi] = -1;
                }
            }
            return zr_http_exchange(vm, (status_code == 303) ? "GET" : method, next_url,
                                    hdr_keys, hdr_vals, nhdrs,
                                    (status_code == 303) ? NULL : body,
                                    (status_code == 303) ? 0 : body_len,
                                    resp_body, out_headers, opt_sink,
                                    redirect_depth + 1, raise_on_http_error, out_status);
        }

        if (raise_on_http_error && status_code >= 400) {
            zr_buf_free(&step_body);
            vm_raise(vm, "http status %d for '%s'", status_code, url);
            return false;
        }
        if (resp_body && step_body.len > 0) {
            zr_buf_append(resp_body, (const uint8_t *)step_body.data, step_body.len);
        }
        zr_buf_free(&step_body);
        return true;
    }

    zr_buf_free(&req);
    return false;
}

static bool zr_http_request_exec(PbVM *vm, const char *method, const char *url,
                                 const char **hdr_keys, const char **hdr_vals, size_t nhdrs,
                                 const uint8_t *body, size_t body_len, ZrBuf *resp_body,
                                 int redirect_depth, bool raise_on_http_error, int *out_status) {
    return zr_http_exchange(vm, method, url, hdr_keys, hdr_vals, nhdrs, body, body_len,
                            resp_body, NULL, NULL, redirect_depth, raise_on_http_error, out_status);
}

static bool zr_extract_headers_arg(PbVM *vm, PbValue hval, const char *fn_name,
                                   const char ***out_hk, const char ***out_hv, size_t *out_nh) {
    *out_hk = NULL;
    *out_hv = NULL;
    *out_nh = 0;
    if (hval.tag == VAL_NIL) return true;
    if (!val_is_obj(hval, OBJ_MAP)) {
        vm_raise(vm, "%s(): headers argument must be map or nil", fn_name);
        return false;
    }
    PbMap *m = (PbMap *)val_as_obj(hval);
    const char **hk = (const char **)zr_xmalloc((m->count + 1) * sizeof(char *));
    const char **hv = (const char **)zr_xmalloc((m->count + 1) * sizeof(char *));
    size_t nh = 0;
    for (size_t i = 0; i < m->used; i++) {
        if (m->entries[i].active && m->entries[i].key && val_is_obj(m->entries[i].val, OBJ_STR)) {
            hk[nh] = m->entries[i].key->chars;
            hv[nh] = ((PbStr *)val_as_obj(m->entries[i].val))->chars;
            nh++;
        }
    }
    *out_hk = hk;
    *out_hv = hv;
    *out_nh = nh;
    return true;
}

static bool bi_http_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_HTTP), "http_get", "http")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "http_get() expects 1 URL string"); return false; }
    if (!zr_check_taint_sink(vm, argv[0], "http_get")) return false;
    ZrBuf resp;
    zr_buf_init(&resp);
    int st = 0;
    if (!zr_http_request_exec(vm, "GET", ((PbStr *)val_as_obj(argv[0]))->chars, NULL, NULL, 0, NULL, 0, &resp, 0, true, &st)) {
        zr_buf_free(&resp);
        return false;
    }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_post(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_HTTP), "http_post", "http")) return false;
    if ((argc != 2 && argc != 3) || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "http_post(url, body[, content_type]) expects strings");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[0], "http_post")) return false;
    if (!zr_check_taint_sink(vm, argv[1], "http_post")) return false;
    const char *ct = "application/json";
    if (argc == 3) {
        if (!val_is_obj(argv[2], OBJ_STR)) { vm_raise(vm, "http_post(): content_type must be string"); return false; }
        ct = ((PbStr *)val_as_obj(argv[2]))->chars;
    }
    const char *hk[1] = { "Content-Type" };
    const char *hv[1] = { ct };
    PbStr *body = (PbStr *)val_as_obj(argv[1]);
    ZrBuf resp;
    zr_buf_init(&resp);
    int st = 0;
    if (!zr_http_request_exec(vm, "POST", ((PbStr *)val_as_obj(argv[0]))->chars,
                              hk, hv, 1, (const uint8_t *)body->chars, body->len, &resp, 0, true, &st)) {
        zr_buf_free(&resp);
        return false;
    }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_get_auth(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_HTTP), "http_get_auth", "http")) return false;
    if (argc < 2 || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "http_get_auth(url, token_or_headers_map) expects (string, string|map)");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[0], "http_get_auth")) return false;
    ZrBuf resp;
    zr_buf_init(&resp);
    int st = 0;
    bool ok = false;
    if (val_is_obj(argv[1], OBJ_MAP)) {
        const char **hk = NULL, **hv = NULL;
        size_t nh = 0;
        zr_extract_headers_arg(vm, argv[1], "http_get_auth", &hk, &hv, &nh);
        ok = zr_http_request_exec(vm, "GET", ((PbStr *)val_as_obj(argv[0]))->chars, hk, hv, nh, NULL, 0, &resp, 0, true, &st);
        free(hk);
        free(hv);
    } else if (val_is_obj(argv[1], OBJ_STR)) {
        PbStr *tok = (PbStr *)val_as_obj(argv[1]);
        char *bearer = (char *)zr_xmalloc(tok->len + 16);
        snprintf(bearer, tok->len + 16, "Bearer %s", tok->chars);
        const char *hk[1] = { "Authorization" };
        const char *hv[1] = { bearer };
        ok = zr_http_request_exec(vm, "GET", ((PbStr *)val_as_obj(argv[0]))->chars, hk, hv, 1, NULL, 0, &resp, 0, true, &st);
        free(bearer);
    } else {
        zr_buf_free(&resp);
        vm_raise(vm, "http_get_auth(url, token_or_headers_map) expects string or map as 2nd argument");
        return false;
    }
    if (!ok) { zr_buf_free(&resp); return false; }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_post_auth(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_HTTP), "http_post_auth", "http")) return false;
    if (argc < 3 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "http_post_auth(url, headers_map, body) expects (string, map, string)");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[0], "http_post_auth")) return false;
    if (!zr_check_taint_sink(vm, argv[1], "http_post_auth")) return false;
    if (!zr_check_taint_sink(vm, argv[2], "http_post_auth")) return false;
    const char **hk = NULL, **hv = NULL;
    size_t nh = 0;
    if (val_is_obj(argv[1], OBJ_MAP)) {
        zr_extract_headers_arg(vm, argv[1], "http_post_auth", &hk, &hv, &nh);
    }
    PbStr *body = (PbStr *)val_as_obj(argv[2]);
    ZrBuf resp;
    zr_buf_init(&resp);
    int st = 0;
    bool ok = zr_http_request_exec(vm, "POST", ((PbStr *)val_as_obj(argv[0]))->chars,
                                   hk, hv, nh, (const uint8_t *)body->chars, body->len, &resp, 0, true, &st);
    free(hk);
    free(hv);
    if (!ok) { zr_buf_free(&resp); return false; }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_request(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    bool has_cap = vm_effective_cap(vm, CAP_HTTP) || vm_effective_cap(vm, CAP_NET);
    if (!vm_check_cap(vm, has_cap, "http_request", "http")) return false;
    if (argc < 2 || argc > 4 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "http_request(method, url[, headers_map[, body]]) expects (string, string[, map|nil[, string|bytes|nil]])");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[1], "http_request")) return false;
    if (argc >= 3 && !zr_check_taint_sink(vm, argv[2], "http_request")) return false;
    if (argc >= 4 && !zr_check_taint_sink(vm, argv[3], "http_request")) return false;

    const char *method = ((PbStr *)val_as_obj(argv[0]))->chars;
    const char *url = ((PbStr *)val_as_obj(argv[1]))->chars;
    const char **hk = NULL, **hv = NULL;
    size_t nh = 0;
    if (argc >= 3 && !zr_extract_headers_arg(vm, argv[2], "http_request", &hk, &hv, &nh)) return false;

    const uint8_t *bptr = NULL;
    size_t blen = 0;
    if (argc >= 4 && argv[3].tag != VAL_NIL) {
        if (val_is_obj(argv[3], OBJ_STR)) {
            PbStr *bs = (PbStr *)val_as_obj(argv[3]);
            bptr = (const uint8_t *)bs->chars;
            blen = bs->len;
        } else if (val_is_obj(argv[3], OBJ_BYTES)) {
            PbBytes *bb = (PbBytes *)val_as_obj(argv[3]);
            bptr = bb->data;
            blen = bb->len;
        } else {
            free(hk); free(hv);
            vm_raise(vm, "http_request(): body must be string, bytes, or nil");
            return false;
        }
    }

    PbMap *resp_hdrs = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(resp_hdrs));

    ZrBuf resp;
    zr_buf_init(&resp);
    int status_code = 0;
    bool ok = zr_http_exchange(vm, method, url, hk, hv, nh, bptr, blen, &resp, resp_hdrs, NULL, 0, false, &status_code);
    free(hk);
    free(hv);
    if (!ok) {
        vm_temp_pop(vm, 1);
        zr_buf_free(&resp);
        return false;
    }

    PbMap *res = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "ok"), val_bool(status_code >= 200 && status_code < 300));
    map_set(res, vm_const_str(vm, "status"), val_num((double)status_code));
    map_set(res, vm_const_str(vm, "headers"), val_obj(resp_hdrs));
    map_set(res, vm_const_str(vm, "body"), val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len)));
    vm_temp_pop(vm, 2);
    zr_buf_free(&resp);
    *out = val_obj(res);
    return true;
}

static bool zr_is_callable_val(PbValue v) {
    if (v.tag == VAL_BUILTIN) return true;
    if (v.tag == VAL_OBJ && v.as.obj) {
        return v.as.obj->type == OBJ_CLOSURE || v.as.obj->type == OBJ_FUNC || v.as.obj->type == OBJ_MAP;
    }
    return false;
}

static bool bi_http_stream_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    bool has_cap = vm_effective_cap(vm, CAP_HTTP) || vm_effective_cap(vm, CAP_NET);
    if (!vm_check_cap(vm, has_cap, "http_stream_get", "http")) return false;
    if ((argc < 2 || argc > 4) || !val_is_obj(argv[0], OBJ_STR) || !zr_is_callable_val(argv[1])) {
        vm_raise(vm, "http_stream_get(url, on_chunk[, headers_map[, timeout_ms]]) expects (string, fn[, map|nil[, number]])");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[0], "http_stream_get")) return false;
    if (argc >= 3 && !zr_check_taint_sink(vm, argv[2], "http_stream_get")) return false;

    const char **hk = NULL, **hv = NULL;
    size_t nh = 0;
    if (argc >= 3 && !zr_extract_headers_arg(vm, argv[2], "http_stream_get", &hk, &hv, &nh)) return false;

    uint32_t saved_timeout = vm->caps.io_timeout_ms;
    if (argc >= 4 && argv[3].tag == VAL_NUM && argv[3].u.num > 0.0) {
        vm->caps.io_timeout_ms = (uint32_t)argv[3].u.num;
    }

    PbMap *resp_hdrs = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(resp_hdrs));
    vm_temp_push(vm, argv[1]);

    ZrHttpStreamSink sink;
    memset(&sink, 0, sizeof(sink));
    sink.has_cb = true;
    sink.on_chunk = argv[1];
    sink.headers_map = resp_hdrs;

    int status_code = 0;
    bool ok = zr_http_exchange(vm, "GET", ((PbStr *)val_as_obj(argv[0]))->chars,
                               hk, hv, nh, NULL, 0, NULL, resp_hdrs, &sink, 0, false, &status_code);
    vm->caps.io_timeout_ms = saved_timeout;
    free(hk);
    free(hv);
    if (!ok) {
        vm_temp_pop(vm, 2);
        return false;
    }

    PbMap *res = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "ok"), val_bool(status_code >= 200 && status_code < 300));
    map_set(res, vm_const_str(vm, "status"), val_num((double)status_code));
    map_set(res, vm_const_str(vm, "headers"), val_obj(resp_hdrs));
    map_set(res, vm_const_str(vm, "bytes_received"), val_num((double)sink.bytes_received));
    map_set(res, vm_const_str(vm, "chunks"), val_num((double)sink.chunks_delivered));
    map_set(res, vm_const_str(vm, "stopped"), val_bool(sink.stopped_by_cb));
    vm_temp_pop(vm, 3);
    *out = val_obj(res);
    return true;
}

static bool bi_http_stream_request(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    bool has_cap = vm_effective_cap(vm, CAP_HTTP) || vm_effective_cap(vm, CAP_NET);
    if (!vm_check_cap(vm, has_cap, "http_stream_request", "http")) return false;

    const char *method_str = "GET";
    PbValue url_val = val_nil();
    PbValue hdrs_val = val_nil();
    PbValue body_val = val_nil();
    PbValue cb_val = val_nil();
    uint32_t saved_timeout = vm->caps.io_timeout_ms;
    uint32_t req_timeout = saved_timeout;

    if ((argc == 1 || argc == 2) && val_is_obj(argv[0], OBJ_MAP)) {
        PbMap *opts = (PbMap *)val_as_obj(argv[0]);
        PbValue mv, tv;
        if (pb_map_get_cstr(opts, "method", &mv) && val_is_obj(mv, OBJ_STR)) {
            method_str = ((PbStr *)val_as_obj(mv))->chars;
        }
        pb_map_get_cstr(opts, "url", &url_val);
        pb_map_get_cstr(opts, "headers", &hdrs_val);
        pb_map_get_cstr(opts, "body", &body_val);
        if (pb_map_get_cstr(opts, "timeout_ms", &tv) && tv.tag == VAL_NUM && tv.u.num > 0.0) {
            req_timeout = (uint32_t)tv.u.num;
        }
        if (argc == 2) {
            cb_val = argv[1];
        } else {
            if (!pb_map_get_cstr(opts, "on_chunk", &cb_val)) {
                pb_map_get_cstr(opts, "callback", &cb_val);
            }
        }
        if (!val_is_obj(url_val, OBJ_STR) || !zr_is_callable_val(cb_val)) {
            vm_raise(vm, "http_stream_request(opts[, on_chunk]) requires opts.url string and callable callback");
            return false;
        }
    } else if (argc == 2 && val_is_obj(argv[0], OBJ_STR) && zr_is_callable_val(argv[1])) {
        url_val = argv[0];
        cb_val = argv[1];
    } else if ((argc == 5 || argc == 6) && val_is_obj(argv[0], OBJ_STR) && val_is_obj(argv[1], OBJ_STR) && zr_is_callable_val(argv[4])) {
        method_str = ((PbStr *)val_as_obj(argv[0]))->chars;
        url_val = argv[1];
        hdrs_val = argv[2];
        body_val = argv[3];
        cb_val = argv[4];
        if (argc == 6 && argv[5].tag == VAL_NUM && argv[5].u.num > 0.0) {
            req_timeout = (uint32_t)argv[5].u.num;
        }
    } else {
        vm_raise(vm, "http_stream_request expects (url_or_opts, on_chunk) or (method, url, headers_or_nil, body_or_nil, on_chunk[, timeout_ms])");
        return false;
    }

    if (!zr_check_taint_sink(vm, url_val, "http_stream_request")) return false;
    if (!zr_check_taint_sink(vm, hdrs_val, "http_stream_request")) return false;
    if (!zr_check_taint_sink(vm, body_val, "http_stream_request")) return false;

    const char **hk = NULL, **hv = NULL;
    size_t nh = 0;
    if (!zr_extract_headers_arg(vm, hdrs_val, "http_stream_request", &hk, &hv, &nh)) return false;

    const uint8_t *bptr = NULL;
    size_t blen = 0;
    if (body_val.tag != VAL_NIL) {
        if (val_is_obj(body_val, OBJ_STR)) {
            PbStr *bs = (PbStr *)val_as_obj(body_val);
            bptr = (const uint8_t *)bs->chars;
            blen = bs->len;
        } else if (val_is_obj(body_val, OBJ_BYTES)) {
            PbBytes *bb = (PbBytes *)val_as_obj(body_val);
            bptr = bb->data;
            blen = bb->len;
        } else {
            free(hk); free(hv);
            vm_raise(vm, "http_stream_request(): body must be string, bytes, or nil");
            return false;
        }
    }

    PbMap *resp_hdrs = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(resp_hdrs));
    vm_temp_push(vm, cb_val);

    ZrHttpStreamSink sink;
    memset(&sink, 0, sizeof(sink));
    sink.has_cb = true;
    sink.on_chunk = cb_val;
    sink.headers_map = resp_hdrs;

    vm->caps.io_timeout_ms = req_timeout;
    int status_code = 0;
    bool ok = zr_http_exchange(vm, method_str,
                               ((PbStr *)val_as_obj(url_val))->chars,
                               hk, hv, nh, bptr, blen, NULL, resp_hdrs, &sink, 0, false, &status_code);
    vm->caps.io_timeout_ms = saved_timeout;
    free(hk);
    free(hv);
    if (!ok) {
        vm_temp_pop(vm, 2);
        return false;
    }

    PbMap *res = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "ok"), val_bool(status_code >= 200 && status_code < 300));
    map_set(res, vm_const_str(vm, "status"), val_num((double)status_code));
    map_set(res, vm_const_str(vm, "headers"), val_obj(resp_hdrs));
    map_set(res, vm_const_str(vm, "bytes_received"), val_num((double)sink.bytes_received));
    map_set(res, vm_const_str(vm, "chunks"), val_num((double)sink.chunks_delivered));
    map_set(res, vm_const_str(vm, "stopped"), val_bool(sink.stopped_by_cb));
    vm_temp_pop(vm, 3);
    *out = val_obj(res);
    return true;
}

typedef struct {
    const char    *field_name;
    const char    *file_path;
    const char    *filename;
    const char    *content_type;
    const uint8_t *mem_data;
    size_t         mem_len;
    bool           is_mem;
    FILE          *fp;
    uint64_t       fsize;
    ZrBuf          part_hdr;
} ZrMpFileSpec;

static const char *zr_mp_basename(const char *path) {
    const char *s1 = strrchr(path, '/');
    const char *s2 = strrchr(path, '\\');
    const char *b = s1;
    if (s2 && (!b || s2 > b)) b = s2;
    return b ? (b + 1) : path;
}

static bool bi_http_upload_multipart(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    bool has_http = vm_effective_cap(vm, CAP_HTTP) || vm_effective_cap(vm, CAP_NET);
    if (!vm_check_cap(vm, has_http, "http_upload_multipart", "http")) return false;
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FS), "http_upload_multipart", "fs")) return false;
    if (argc < 3 || argc > 5 || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "http_upload_multipart(url, fields_map_or_nil, files_map_or_array[, headers_map[, timeout_ms]]) expects 3..5 arguments");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[0], "http_upload_multipart")) return false;
    if (!zr_check_taint_sink(vm, argv[1], "http_upload_multipart")) return false;
    if (!zr_check_taint_sink(vm, argv[2], "http_upload_multipart")) return false;
    if (argc >= 4 && !zr_check_taint_sink(vm, argv[3], "http_upload_multipart")) return false;

    uint32_t eff_timeout_ms = vm->caps.io_timeout_ms;
    if (argc >= 5 && argv[4].tag == VAL_NUM && argv[4].u.num > 0.0) {
        eff_timeout_ms = (uint32_t)argv[4].u.num;
    }

    const char *url = ((PbStr *)val_as_obj(argv[0]))->chars;
    bool is_https = false;
    char host[512], path[2048];
    uint16_t port = 80;
    if (!zr_parse_url(url, &is_https, host, &port, path)) {
        vm_raise(vm, "http_upload_multipart(): invalid URL '%s'", url);
        return false;
    }

    uint8_t rnd[16];
    zr_csprng_bytes(rnd, sizeof(rnd));
    char boundary[64];
    int boff = snprintf(boundary, sizeof(boundary), "----PBLangBoundary");
    for (size_t i = 0; i < sizeof(rnd); i++) {
        boff += snprintf(boundary + boff, sizeof(boundary) - (size_t)boff, "%02x", rnd[i]);
    }

    /* Build form fields preamble */
    ZrBuf fields_buf;
    zr_buf_init(&fields_buf);
    if (argv[1].tag != VAL_NIL) {
        if (!val_is_obj(argv[1], OBJ_MAP)) {
            zr_buf_free(&fields_buf);
            vm_raise(vm, "http_upload_multipart(): fields must be map or nil");
            return false;
        }
        PbMap *fm = (PbMap *)val_as_obj(argv[1]);
        for (size_t i = 0; i < fm->used; i++) {
            if (!fm->entries[i].active || !fm->entries[i].key) continue;
            char *vs = vm_value_to_cstr(vm, fm->entries[i].val, NULL);
            zr_buf_append_cstr(&fields_buf, "--");
            zr_buf_append_cstr(&fields_buf, boundary);
            zr_buf_append_cstr(&fields_buf, "\r\nContent-Disposition: form-data; name=\"");
            zr_buf_append_cstr(&fields_buf, fm->entries[i].key->chars);
            zr_buf_append_cstr(&fields_buf, "\"\r\n\r\n");
            zr_buf_append_cstr(&fields_buf, vs);
            zr_buf_append_cstr(&fields_buf, "\r\n");
            free(vs);
        }
    }

    /* Collect and open file entries to compute exact Content-Length */
    ZrMpFileSpec *fspecs = NULL;
    size_t nf = 0;
    if (val_is_obj(argv[2], OBJ_MAP)) {
        PbMap *flm = (PbMap *)val_as_obj(argv[2]);
        fspecs = (ZrMpFileSpec *)zr_xcalloc(flm->count + 1, sizeof(ZrMpFileSpec));
        for (size_t i = 0; i < flm->used; i++) {
            if (!flm->entries[i].active || !flm->entries[i].key) continue;
            const char *fname_field = flm->entries[i].key->chars;
            PbValue fv = flm->entries[i].val;
            const char *fpath = NULL, *disp_name = NULL, *ctype = "application/octet-stream";
            const uint8_t *mdata = NULL;
            size_t mlen = 0;
            bool is_mem = false;
            if (val_is_obj(fv, OBJ_STR)) {
                fpath = ((PbStr *)val_as_obj(fv))->chars;
            } else if (val_is_obj(fv, OBJ_MAP)) {
                PbMap *sm = (PbMap *)val_as_obj(fv);
                PbValue pv, nv, cv, dv;
                if (pb_map_get_cstr(sm, "path", &pv) && val_is_obj(pv, OBJ_STR)) fpath = ((PbStr *)val_as_obj(pv))->chars;
                if (pb_map_get_cstr(sm, "filename", &nv) && val_is_obj(nv, OBJ_STR)) disp_name = ((PbStr *)val_as_obj(nv))->chars;
                if (pb_map_get_cstr(sm, "content_type", &cv) && val_is_obj(cv, OBJ_STR)) ctype = ((PbStr *)val_as_obj(cv))->chars;
                if (!fpath && pb_map_get_cstr(sm, "data", &dv)) {
                    if (val_is_obj(dv, OBJ_STR)) {
                        PbStr *ds = (PbStr *)val_as_obj(dv);
                        mdata = (const uint8_t *)ds->chars;
                        mlen = ds->len;
                        is_mem = true;
                    } else if (val_is_obj(dv, OBJ_BYTES)) {
                        PbBytes *db = (PbBytes *)val_as_obj(dv);
                        mdata = db->data;
                        mlen = db->len;
                        is_mem = true;
                    }
                }
            }
            if (!fpath && !is_mem) continue;
            fspecs[nf].field_name = fname_field;
            fspecs[nf].file_path = fpath;
            fspecs[nf].filename = disp_name ? disp_name : (fpath ? zr_mp_basename(fpath) : fname_field);
            fspecs[nf].content_type = ctype;
            fspecs[nf].mem_data = mdata;
            fspecs[nf].mem_len = mlen;
            fspecs[nf].is_mem = is_mem;
            nf++;
        }
    } else if (val_is_obj(argv[2], OBJ_ARRAY)) {
        PbArray *fla = (PbArray *)val_as_obj(argv[2]);
        fspecs = (ZrMpFileSpec *)zr_xcalloc(fla->len + 1, sizeof(ZrMpFileSpec));
        for (size_t i = 0; i < fla->len; i++) {
            if (!val_is_obj(fla->items[i], OBJ_MAP)) continue;
            PbMap *sm = (PbMap *)val_as_obj(fla->items[i]);
            PbValue nmv, pv, fnv, cv, dv;
            const char *fld = "file", *fpath = NULL, *disp = NULL, *ctype = "application/octet-stream";
            const uint8_t *mdata = NULL;
            size_t mlen = 0;
            bool is_mem = false;
            if (pb_map_get_cstr(sm, "name", &nmv) && val_is_obj(nmv, OBJ_STR)) fld = ((PbStr *)val_as_obj(nmv))->chars;
            else if (pb_map_get_cstr(sm, "field", &nmv) && val_is_obj(nmv, OBJ_STR)) fld = ((PbStr *)val_as_obj(nmv))->chars;
            if (pb_map_get_cstr(sm, "path", &pv) && val_is_obj(pv, OBJ_STR)) fpath = ((PbStr *)val_as_obj(pv))->chars;
            if (pb_map_get_cstr(sm, "filename", &fnv) && val_is_obj(fnv, OBJ_STR)) disp = ((PbStr *)val_as_obj(fnv))->chars;
            if (pb_map_get_cstr(sm, "content_type", &cv) && val_is_obj(cv, OBJ_STR)) ctype = ((PbStr *)val_as_obj(cv))->chars;
            if (!fpath && pb_map_get_cstr(sm, "data", &dv)) {
                if (val_is_obj(dv, OBJ_STR)) {
                    PbStr *ds = (PbStr *)val_as_obj(dv);
                    mdata = (const uint8_t *)ds->chars;
                    mlen = ds->len;
                    is_mem = true;
                } else if (val_is_obj(dv, OBJ_BYTES)) {
                    PbBytes *db = (PbBytes *)val_as_obj(dv);
                    mdata = db->data;
                    mlen = db->len;
                    is_mem = true;
                }
            }
            if (!fpath && !is_mem) continue;
            fspecs[nf].field_name = fld;
            fspecs[nf].file_path = fpath;
            fspecs[nf].filename = disp ? disp : (fpath ? zr_mp_basename(fpath) : fld);
            fspecs[nf].content_type = ctype;
            fspecs[nf].mem_data = mdata;
            fspecs[nf].mem_len = mlen;
            fspecs[nf].is_mem = is_mem;
            nf++;
        }
    } else if (argv[2].tag != VAL_NIL) {
        zr_buf_free(&fields_buf);
        vm_raise(vm, "http_upload_multipart(): files must be map, array, or nil");
        return false;
    }

    uint64_t total_len = (uint64_t)fields_buf.len;
    for (size_t i = 0; i < nf; i++) {
        if (fspecs[i].is_mem) {
            fspecs[i].fp = NULL;
            fspecs[i].fsize = (uint64_t)fspecs[i].mem_len;
        } else {
            FILE *fp = fopen(fspecs[i].file_path, "rb");
            if (!fp) {
                const char *bad_path = fspecs[i].file_path;
                for (size_t k = 0; k < i; k++) {
                    if (fspecs[k].fp) fclose(fspecs[k].fp);
                    zr_buf_free(&fspecs[k].part_hdr);
                }
                free(fspecs);
                zr_buf_free(&fields_buf);
                vm_raise(vm, "http_upload_multipart(): cannot open '%s'", bad_path);
                return false;
            }
#if defined(_WIN32)
            _fseeki64(fp, 0, SEEK_END);
            long long sz = _ftelli64(fp);
            _fseeki64(fp, 0, SEEK_SET);
#else
            fseeko(fp, 0, SEEK_END);
            off_t sz = ftello(fp);
            fseeko(fp, 0, SEEK_SET);
#endif
            fspecs[i].fp = fp;
            fspecs[i].fsize = (sz > 0) ? (uint64_t)sz : 0;
        }
        zr_buf_init(&fspecs[i].part_hdr);
        zr_buf_append_cstr(&fspecs[i].part_hdr, "--");
        zr_buf_append_cstr(&fspecs[i].part_hdr, boundary);
        zr_buf_append_cstr(&fspecs[i].part_hdr, "\r\nContent-Disposition: form-data; name=\"");
        zr_buf_append_cstr(&fspecs[i].part_hdr, fspecs[i].field_name);
        zr_buf_append_cstr(&fspecs[i].part_hdr, "\"; filename=\"");
        zr_buf_append_cstr(&fspecs[i].part_hdr, fspecs[i].filename);
        zr_buf_append_cstr(&fspecs[i].part_hdr, "\"\r\nContent-Type: ");
        zr_buf_append_cstr(&fspecs[i].part_hdr, fspecs[i].content_type);
        zr_buf_append_cstr(&fspecs[i].part_hdr, "\r\n\r\n");
        total_len += (uint64_t)fspecs[i].part_hdr.len + fspecs[i].fsize + 2ULL; /* +\r\n */
    }
    char closing_bnd[80];
    int cblen = snprintf(closing_bnd, sizeof(closing_bnd), "--%s--\r\n", boundary);
    total_len += (uint64_t)cblen;

    const char **hk = NULL, **hv = NULL;
    size_t nh = 0;
    if (argc >= 4 && !zr_extract_headers_arg(vm, argv[3], "http_upload_multipart", &hk, &hv, &nh)) {
        for (size_t k = 0; k < nf; k++) { if (fspecs[k].fp) fclose(fspecs[k].fp); zr_buf_free(&fspecs[k].part_hdr); }
        free(fspecs);
        zr_buf_free(&fields_buf);
        return false;
    }

    /* Connect or reuse pooled connection and stream multipart body in 64 KiB chunks */
    ZrNetEntry tmp_e;
    memset(&tmp_e, 0, sizeof(tmp_e));
    tmp_e.sock = ZR_INVALID_SOCK;
    char port_s[16], err[256];
    snprintf(port_s, sizeof(port_s), "%u", (unsigned)port);
    err[0] = '\0';

    bool reused = zr_http_pool_acquire(host, port, is_https, eff_timeout_ms, &tmp_e);
    if (!reused) {
        ZrSock s = zr_connect_tcp_raw(host, port_s, eff_timeout_ms, err);
        if (s == ZR_INVALID_SOCK) {
            for (size_t k = 0; k < nf; k++) { if (fspecs[k].fp) fclose(fspecs[k].fp); zr_buf_free(&fspecs[k].part_hdr); }
            free(fspecs); free(hk); free(hv); zr_buf_free(&fields_buf);
            vm_raise(vm, "http_upload_multipart(): %s", err);
            return false;
        }
        tmp_e.kind = ZR_NET_STREAM;
        tmp_e.sock = s;
        if (is_https && !zr_tls_client_wrap(s, host, &tmp_e.ssl_ctx, &tmp_e.ssl, err)) {
            zr_close_raw_sock(s);
            for (size_t k = 0; k < nf; k++) { if (fspecs[k].fp) fclose(fspecs[k].fp); zr_buf_free(&fspecs[k].part_hdr); }
            free(fspecs); free(hk); free(hv); zr_buf_free(&fields_buf);
            vm_raise(vm, "http_upload_multipart(): %s", err);
            return false;
        }
    }

    ZrBuf req_hdr;
    zr_buf_init(&req_hdr);
    char head_top[3072];
    if ((is_https && port == 443) || (!is_https && port == 80)) {
        snprintf(head_top, sizeof(head_top),
                 "POST %s HTTP/1.1\r\nHost: %s\r\nUser-Agent: pblang/4.2\r\nConnection: keep-alive\r\n"
                 "Content-Type: multipart/form-data; boundary=%s\r\nContent-Length: %llu\r\n",
                 path, host, boundary, (unsigned long long)total_len);
    } else {
        snprintf(head_top, sizeof(head_top),
                 "POST %s HTTP/1.1\r\nHost: %s:%u\r\nUser-Agent: pblang/4.2\r\nConnection: keep-alive\r\n"
                 "Content-Type: multipart/form-data; boundary=%s\r\nContent-Length: %llu\r\n",
                 path, host, (unsigned)port, boundary, (unsigned long long)total_len);
    }
    zr_buf_append_cstr(&req_hdr, head_top);
    for (size_t i = 0; i < nh; i++) {
        if (!hk[i] || !hv[i]) continue;
        if (zr_ieq_str(hk[i], "Content-Type") || zr_ieq_str(hk[i], "Content-Length")) continue;
        zr_buf_append_cstr(&req_hdr, hk[i]);
        zr_buf_append_cstr(&req_hdr, ": ");
        zr_buf_append_cstr(&req_hdr, hv[i]);
        zr_buf_append_cstr(&req_hdr, "\r\n");
    }
    zr_buf_append_cstr(&req_hdr, "\r\n");
    free(hk);
    free(hv);

    /* If pooled socket broke right on header write, reconnect once */
    if (zr_stream_write_all(&tmp_e, (const uint8_t *)req_hdr.data, req_hdr.len, err) < 0 && reused) {
        zr_net_destroy_entry(&tmp_e);
        ZrSock s = zr_connect_tcp_raw(host, port_s, eff_timeout_ms, err);
        if (s != ZR_INVALID_SOCK) {
            tmp_e.kind = ZR_NET_STREAM;
            tmp_e.sock = s;
            if (!is_https || zr_tls_client_wrap(s, host, &tmp_e.ssl_ctx, &tmp_e.ssl, err)) {
                err[0] = '\0';
                zr_stream_write_all(&tmp_e, (const uint8_t *)req_hdr.data, req_hdr.len, err);
            }
        }
    }
    zr_buf_free(&req_hdr);

    bool write_ok = (err[0] == '\0');
    if (write_ok && fields_buf.len > 0) {
        write_ok = (zr_stream_write_all(&tmp_e, (const uint8_t *)fields_buf.data, fields_buf.len, err) >= 0);
    }
    zr_buf_free(&fields_buf);

    uint8_t *io_chunk = (uint8_t *)zr_xmalloc(65536);
    for (size_t i = 0; i < nf && write_ok; i++) {
        if (zr_stream_write_all(&tmp_e, (const uint8_t *)fspecs[i].part_hdr.data, fspecs[i].part_hdr.len, err) < 0) {
            write_ok = false;
            break;
        }
        if (fspecs[i].is_mem) {
            if (fspecs[i].mem_len > 0 &&
                zr_stream_write_all(&tmp_e, fspecs[i].mem_data, fspecs[i].mem_len, err) < 0) {
                write_ok = false;
                break;
            }
        } else {
            uint64_t rem = fspecs[i].fsize;
            while (rem > 0) {
                size_t want = rem < 65536ULL ? (size_t)rem : 65536;
                size_t got = fread(io_chunk, 1, want, fspecs[i].fp);
                if (got == 0) {
                    memset(io_chunk, 0, want);
                    got = want;
                }
                if (zr_stream_write_all(&tmp_e, io_chunk, got, err) < 0) {
                    write_ok = false;
                    break;
                }
                rem -= (uint64_t)got;
            }
        }
        if (write_ok && zr_stream_write_all(&tmp_e, (const uint8_t *)"\r\n", 2, err) < 0) write_ok = false;
    }
    free(io_chunk);
    for (size_t i = 0; i < nf; i++) {
        if (fspecs[i].fp) fclose(fspecs[i].fp);
        zr_buf_free(&fspecs[i].part_hdr);
    }
    free(fspecs);

    if (write_ok) {
        write_ok = (zr_stream_write_all(&tmp_e, (const uint8_t *)closing_bnd, (size_t)cblen, err) >= 0);
    }
    if (!write_ok) {
        zr_net_destroy_entry(&tmp_e);
        vm_raise(vm, "http_upload_multipart() write failed: %s", err);
        return false;
    }

    PbMap *resp_hdrs = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(resp_hdrs));
    ZrBuf resp_body;
    zr_buf_init(&resp_body);
    ZrHttpStreamSink sink;
    memset(&sink, 0, sizeof(sink));
    sink.on_chunk = val_nil();
    sink.headers_map = resp_hdrs;

    int status_code = 0;
    bool zero_eof = false;
    if (!zr_http_read_response_on_entry(vm, &tmp_e, "POST", url, host, port, is_https, false,
                                        &resp_body, &sink, resp_hdrs, &status_code, NULL, &zero_eof, err)) {
        zr_net_destroy_entry(&tmp_e);
        zr_buf_free(&resp_body);
        vm_temp_pop(vm, 1);
        vm_raise(vm, "http_upload_multipart() response failed: %s", err);
        return false;
    }

    PbMap *res = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "ok"), val_bool(status_code >= 200 && status_code < 300));
    map_set(res, vm_const_str(vm, "status"), val_num((double)status_code));
    map_set(res, vm_const_str(vm, "headers"), val_obj(resp_hdrs));
    map_set(res, vm_const_str(vm, "body"), val_obj(vm_new_str(vm, (const char *)(resp_body.data ? resp_body.data : ""), resp_body.len)));
    map_set(res, vm_const_str(vm, "bytes_uploaded"), val_num((double)total_len));
    vm_temp_pop(vm, 2);
    zr_buf_free(&resp_body);
    *out = val_obj(res);
    return true;
}

/* ---- Native Non-Blocking / Timeout-Bounded UDP Socket Builtins ---- */
static bool bi_udp_bind(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "udp_bind", "net")) return false;
    if ((argc != 1 && argc != 2) || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "udp_bind(host, port) or udp_bind('host:port') expects 1 or 2 arguments");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[0], "udp_bind")) return false;
    if (argc == 2 && !zr_check_taint_sink(vm, argv[1], "udp_bind")) return false;
#if defined(__wasi__)
    (void)out;
    vm_raise(vm, "udp_bind(): unavailable in WASI sandbox");
    return false;
#else
    zr_net_ensure_init();
    char host[512], port[32], desc[560];
    if (argc == 2) {
        const char *h = ((PbStr *)val_as_obj(argv[0]))->chars;
        snprintf(host, sizeof(host), "%.500s", (h && h[0]) ? h : "0.0.0.0");
        if (argv[1].tag == VAL_NUM) {
            long p = (long)argv[1].u.num;
            if (p < 0 || p > 65535) {
                vm_raise(vm, "udp_bind(): port %ld out of range (0..65535)", p);
                return false;
            }
            snprintf(port, sizeof(port), "%ld", p);
        } else if (val_is_obj(argv[1], OBJ_STR)) {
            snprintf(port, sizeof(port), "%.30s", ((PbStr *)val_as_obj(argv[1]))->chars);
        } else {
            vm_raise(vm, "udp_bind(host, port): port must be number or string");
            return false;
        }
        snprintf(desc, sizeof(desc), "%s:%s", host, port);
    } else {
        const char *addr = ((PbStr *)val_as_obj(argv[0]))->chars;
        snprintf(desc, sizeof(desc), "%.550s", addr);
        if (addr[0] == ':' && addr[1] != '\0') {
            snprintf(host, sizeof(host), "0.0.0.0");
            snprintf(port, sizeof(port), "%.30s", addr + 1);
        } else if (!zr_split_host_port(addr, host, port)) {
            vm_raise(vm, "udp_bind(): invalid address '%s' (expected host:port)", addr);
            return false;
        }
    }
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_DGRAM;
    hints.ai_flags = AI_PASSIVE;
    if (getaddrinfo(host[0] ? host : NULL, port, &hints, &res) != 0 || !res) {
        vm_raise(vm, "udp_bind(): getaddrinfo failed for '%s'", desc);
        return false;
    }
    ZrSock s = ZR_INVALID_SOCK;
    for (struct addrinfo *ai = res; ai; ai = ai->ai_next) {
        s = socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol);
        if (s == ZR_INVALID_SOCK) continue;
        int opt = 1;
        setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));
        if (bind(s, ai->ai_addr, (int)ai->ai_addrlen) == 0) break;
        zr_close_raw_sock(s);
        s = ZR_INVALID_SOCK;
    }
    freeaddrinfo(res);
    if (s == ZR_INVALID_SOCK) {
        vm_raise(vm, "udp_bind(): bind failed for '%s'", desc);
        return false;
    }
    zr_set_sock_timeouts(s, vm->caps.io_timeout_ms);
    size_t id = zr_net_insert(ZR_NET_UDP, s, NULL, NULL);
    if (!id) {
        zr_close_raw_sock(s);
        vm_raise(vm, "udp_bind(): handle table full");
        return false;
    }
    *out = val_num((double)id);
    return true;
#endif
}

static bool bi_udp_local_addr(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "udp_local_addr", "net")) return false;
    if (argc != 1) { vm_raise(vm, "udp_local_addr(handle) expects 1 argument"); return false; }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "udp_local_addr", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_UDP) {
        if (e) zr_net_checkin(e);
        vm_raise(vm, "udp_local_addr(): invalid UDP handle");
        return false;
    }
#if defined(__wasi__)
    zr_net_checkin(e);
    (void)out;
    vm_raise(vm, "udp_local_addr(): unavailable in WASI sandbox");
    return false;
#else
    struct sockaddr_storage ss;
#if defined(_WIN32)
    int slen = (int)sizeof(ss);
#else
    socklen_t slen = (socklen_t)sizeof(ss);
#endif
    if (getsockname(e->sock, (struct sockaddr *)&ss, &slen) != 0) {
        zr_net_checkin(e);
        vm_raise(vm, "udp_local_addr(): getsockname failed");
        return false;
    }
    zr_net_checkin(e);
    char hbuf[NI_MAXHOST], sbuf[NI_MAXSERV];
    hbuf[0] = '\0';
    sbuf[0] = '\0';
    if (getnameinfo((struct sockaddr *)&ss, slen, hbuf, sizeof(hbuf), sbuf, sizeof(sbuf),
                    NI_NUMERICHOST | NI_NUMERICSERV) != 0) {
        vm_raise(vm, "udp_local_addr(): getnameinfo failed");
        return false;
    }
    char full_addr[NI_MAXHOST + NI_MAXSERV + 8];
    if (strchr(hbuf, ':')) snprintf(full_addr, sizeof(full_addr), "[%s]:%s", hbuf, sbuf);
    else snprintf(full_addr, sizeof(full_addr), "%s:%s", hbuf, sbuf);

    PbMap *res = vm_new_map(vm, 4);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "host"), val_obj(vm_new_cstr(vm, hbuf)));
    map_set(res, vm_const_str(vm, "port"), val_num((double)strtol(sbuf, NULL, 10)));
    map_set(res, vm_const_str(vm, "addr"), val_obj(vm_new_cstr(vm, full_addr)));
    vm_temp_pop(vm, 1);
    *out = val_obj(res);
    return true;
#endif
}

static bool bi_udp_set_timeout(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "udp_set_timeout", "net")) return false;
    if (argc != 2 || argv[1].tag != VAL_NUM) {
        vm_raise(vm, "udp_set_timeout(handle, timeout_ms) expects (handle, number)");
        return false;
    }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "udp_set_timeout", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_UDP) {
        if (e) zr_net_checkin(e);
        vm_raise(vm, "udp_set_timeout(): invalid UDP handle");
        return false;
    }
    double ms_d = argv[1].u.num;
    uint64_t ms = (ms_d > 0.0) ? (uint64_t)ms_d : 0ULL;
#if defined(_WIN32)
    DWORD tv = (DWORD)ms;
    setsockopt(e->sock, SOL_SOCKET, SO_RCVTIMEO, (const char *)&tv, sizeof(tv));
    setsockopt(e->sock, SOL_SOCKET, SO_SNDTIMEO, (const char *)&tv, sizeof(tv));
#elif !defined(__wasi__)
    struct timeval tv;
    tv.tv_sec = (time_t)(ms / 1000ULL);
    tv.tv_usec = (suseconds_t)((ms % 1000ULL) * 1000ULL);
    setsockopt(e->sock, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    setsockopt(e->sock, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
#endif
    zr_net_checkin(e);
    *out = val_bool(true);
    return true;
}

static bool bi_udp_sendto(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "udp_sendto", "net")) return false;
    if ((argc != 3 && argc != 4) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "udp_sendto(handle, data, dst_host, dst_port) or udp_sendto(handle, data, 'host:port') expects 3 or 4 arguments");
        return false;
    }
    if (!zr_check_taint_sink(vm, argv[1], "udp_sendto")) return false;
    if (!zr_check_taint_sink(vm, argv[2], "udp_sendto")) return false;
    if (argc == 4 && !zr_check_taint_sink(vm, argv[3], "udp_sendto")) return false;
#if defined(__wasi__)
    (void)out;
    vm_raise(vm, "udp_sendto(): unavailable in WASI sandbox");
    return false;
#else
    const uint8_t *ptr = NULL;
    uint8_t *owned_arr = NULL;
    size_t len = 0;
    if (val_is_obj(argv[1], OBJ_STR)) {
        PbStr *s = (PbStr *)val_as_obj(argv[1]);
        ptr = (const uint8_t *)s->chars;
        len = s->len;
    } else if (val_is_obj(argv[1], OBJ_BYTES)) {
        PbBytes *b = (PbBytes *)val_as_obj(argv[1]);
        ptr = b->data;
        len = b->len;
    } else if (val_is_obj(argv[1], OBJ_ARRAY)) {
        PbArray *a = (PbArray *)val_as_obj(argv[1]);
        owned_arr = (uint8_t *)zr_xmalloc(a->len ? a->len : 1);
        for (size_t bi = 0; bi < a->len; bi++) {
            if (a->items[bi].tag != VAL_NUM) {
                free(owned_arr);
                vm_raise(vm, "udp_sendto(): byte array elements must be numbers (0..255)");
                return false;
            }
            owned_arr[bi] = (uint8_t)(((int)a->items[bi].u.num) & 0xFF);
        }
        ptr = owned_arr;
        len = a->len;
    } else {
        vm_raise(vm, "udp_sendto(): payload must be string, bytes, or byte array");
        return false;
    }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "udp_sendto", &id)) { free(owned_arr); return false; }
    char host[512], port[32], desc[560];
    if (argc == 4) {
        snprintf(host, sizeof(host), "%.500s", ((PbStr *)val_as_obj(argv[2]))->chars);
        if (argv[3].tag == VAL_NUM) {
            long p = (long)argv[3].u.num;
            if (p < 1 || p > 65535) {
                free(owned_arr);
                vm_raise(vm, "udp_sendto(): port %ld out of range (1..65535)", p);
                return false;
            }
            snprintf(port, sizeof(port), "%ld", p);
        } else if (val_is_obj(argv[3], OBJ_STR)) {
            snprintf(port, sizeof(port), "%.30s", ((PbStr *)val_as_obj(argv[3]))->chars);
        } else {
            free(owned_arr);
            vm_raise(vm, "udp_sendto(): dst_port must be number or string");
            return false;
        }
        snprintf(desc, sizeof(desc), "%s:%s", host, port);
    } else {
        const char *target = ((PbStr *)val_as_obj(argv[2]))->chars;
        snprintf(desc, sizeof(desc), "%.550s", target);
        if (!zr_split_host_port(target, host, port)) {
            free(owned_arr);
            vm_raise(vm, "udp_sendto(): invalid target address '%s'", target);
            return false;
        }
    }
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_UDP) {
        if (e) zr_net_checkin(e);
        free(owned_arr);
        vm_raise(vm, "udp_sendto(): invalid UDP handle");
        return false;
    }
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_DGRAM;
    if (getaddrinfo(host, port, &hints, &res) != 0 || !res) {
        zr_net_checkin(e);
        free(owned_arr);
        vm_raise(vm, "udp_sendto(): DNS lookup failed for '%s'", desc);
        return false;
    }
    ssize_t sent = -1;
    for (struct addrinfo *ai = res; ai; ai = ai->ai_next) {
#if defined(_WIN32)
        sent = (ssize_t)sendto(e->sock, (const char *)ptr, (int)len, 0, ai->ai_addr, (int)ai->ai_addrlen);
#else
        do {
            sent = sendto(e->sock, ptr, len, 0, ai->ai_addr, ai->ai_addrlen);
        } while (sent < 0 && errno == EINTR);
#endif
        if (sent >= 0) break;
    }
    freeaddrinfo(res);
    zr_net_checkin(e);
    free(owned_arr);
    if (sent < 0) {
        vm_raise(vm, "udp_sendto(): sendto failed for '%s'", desc);
        return false;
    }
    *out = val_num((double)sent);
    return true;
#endif
}

static bool bi_udp_recvfrom(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "udp_recvfrom", "net")) return false;
    if (argc != 1 && argc != 2) {
        vm_raise(vm, "udp_recvfrom(handle[, max_bytes]) expects 1 or 2 arguments");
        return false;
    }
    size_t max_b = 65535;
    if (argc == 2) {
        if (argv[1].tag != VAL_NUM || argv[1].u.num < 1.0) {
            vm_raise(vm, "udp_recvfrom(): max_bytes must be >= 1");
            return false;
        }
        max_b = (size_t)argv[1].u.num;
        if (max_b > 65536) max_b = 65536;
    }
#if defined(__wasi__)
    (void)out;
    vm_raise(vm, "udp_recvfrom(): unavailable in WASI sandbox");
    return false;
#else
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "udp_recvfrom", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e || e->kind != ZR_NET_UDP) {
        if (e) zr_net_checkin(e);
        vm_raise(vm, "udp_recvfrom(): invalid UDP handle");
        return false;
    }
    uint8_t *buf = (uint8_t *)zr_xmalloc(max_b);
    struct sockaddr_storage ss;
    memset(&ss, 0, sizeof(ss));
#if defined(_WIN32)
    int slen = (int)sizeof(ss);
    int n = recvfrom(e->sock, (char *)buf, (int)max_b, 0, (struct sockaddr *)&ss, &slen);
    if (n < 0) {
        int werr = WSAGetLastError();
        zr_net_checkin(e);
        free(buf);
        if (werr == WSAETIMEDOUT || werr == WSAEWOULDBLOCK) {
            *out = val_nil();
            return true;
        }
        vm_raise(vm, "udp_recvfrom(): recvfrom failed (%d)", werr);
        return false;
    }
#else
    socklen_t slen = (socklen_t)sizeof(ss);
    ssize_t n;
    do {
        n = recvfrom(e->sock, buf, max_b, 0, (struct sockaddr *)&ss, &slen);
    } while (n < 0 && errno == EINTR);
    if (n < 0) {
        int errn = errno;
        zr_net_checkin(e);
        free(buf);
        if (errn == EAGAIN || errn == EWOULDBLOCK) {
            *out = val_nil();
            return true;
        }
        vm_raise(vm, "udp_recvfrom(): %s", strerror(errn));
        return false;
    }
#endif
    zr_net_checkin(e);

    char hbuf[NI_MAXHOST], sbuf[NI_MAXSERV];
    hbuf[0] = '\0';
    sbuf[0] = '\0';
    getnameinfo((struct sockaddr *)&ss, slen, hbuf, sizeof(hbuf), sbuf, sizeof(sbuf),
                NI_NUMERICHOST | NI_NUMERICSERV);
    char full_addr[NI_MAXHOST + NI_MAXSERV + 8];
    if (strchr(hbuf, ':')) snprintf(full_addr, sizeof(full_addr), "[%s]:%s", hbuf, sbuf);
    else snprintf(full_addr, sizeof(full_addr), "%s:%s", hbuf, sbuf);

    PbMap *res = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(res));
    PbBytes *bobj = pb_bytes_new(vm, (size_t)n, false);
    if (n > 0) memcpy(bobj->data, buf, (size_t)n);
    PbValue bval = val_obj(bobj);
    vm_temp_push(vm, bval);
    map_set(res, vm_const_str(vm, "data"), bval);
    map_set(res, vm_const_str(vm, "bytes"), bval);
    map_set(res, vm_const_str(vm, "text"), val_obj(vm_new_str(vm, (const char *)buf, (size_t)n)));
    map_set(res, vm_const_str(vm, "addr"), val_obj(vm_new_cstr(vm, full_addr)));
    map_set(res, vm_const_str(vm, "host"), val_obj(vm_new_cstr(vm, hbuf)));
    map_set(res, vm_const_str(vm, "port"), val_num((double)strtol(sbuf, NULL, 10)));
    map_set(res, vm_const_str(vm, "len"), val_num((double)n));
    map_set(res, vm_const_str(vm, "size"), val_num((double)n));
    vm_temp_pop(vm, 2);
    free(buf);
    *out = val_obj(res);
    return true;
#endif
}

static bool bi_udp_close(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "udp_close", "net")) return false;
    if (argc != 1) { vm_raise(vm, "udp_close(handle) expects 1 argument"); return false; }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "udp_close", &id)) return false;
    ZrNetEntry *e = zr_net_checkout(id);
    if (e) {
        zr_mutex_lock(&g_zr_net_mtx);
        zr_net_destroy_entry(e);
        zr_mutex_unlock(&g_zr_net_mtx);
    }
    *out = val_bool(true);
    return true;
}

/* ---- HTTP Server Helpers, SSE & RFC 6455 WebSocket ---- */
static const char *zr_http_reason(int status) {
    switch (status) {
        case 100: return "Continue";
        case 101: return "Switching Protocols";
        case 200: return "OK";
        case 201: return "Created";
        case 202: return "Accepted";
        case 204: return "No Content";
        case 206: return "Partial Content";
        case 301: return "Moved Permanently";
        case 302: return "Found";
        case 304: return "Not Modified";
        case 307: return "Temporary Redirect";
        case 308: return "Permanent Redirect";
        case 400: return "Bad Request";
        case 401: return "Unauthorized";
        case 403: return "Forbidden";
        case 404: return "Not Found";
        case 405: return "Method Not Allowed";
        case 409: return "Conflict";
        case 413: return "Content Too Large";
        case 418: return "I'm a teapot";
        case 422: return "Unprocessable Content";
        case 429: return "Too Many Requests";
        case 500: return "Internal Server Error";
        case 501: return "Not Implemented";
        case 502: return "Bad Gateway";
        case 503: return "Service Unavailable";
        case 504: return "Gateway Timeout";
        default:  return "Unknown";
    }
}

static bool bi_http_parse_request(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "http_parse_request", "net")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "http_parse_request() expects 1 string"); return false; }
    PbStr *raw = (PbStr *)val_as_obj(argv[0]);
    const char *s = raw->chars;
    const char *sep = strstr(s, "\r\n\r\n");
    size_t head_len = sep ? (size_t)(sep - s) : raw->len;
    const char *body_ptr = sep ? (sep + 4) : (s + raw->len);
    size_t body_len = sep ? (raw->len - head_len - 4) : 0;

    /* Request line */
    size_t rline_end = 0;
    while (rline_end + 1 < head_len && !(s[rline_end] == '\r' && s[rline_end + 1] == '\n')) rline_end++;
    if (rline_end + 1 >= head_len) rline_end = head_len;

    size_t m0 = 0; while (m0 < rline_end && s[m0] == ' ') m0++;
    size_t m1 = m0; while (m1 < rline_end && s[m1] != ' ' && s[m1] != '\t') m1++;
    size_t p0 = m1; while (p0 < rline_end && (s[p0] == ' ' || s[p0] == '\t')) p0++;
    size_t p1 = p0; while (p1 < rline_end && s[p1] != ' ' && s[p1] != '\t') p1++;

    PbMap *headers = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(headers));

    size_t cur = (rline_end + 2 <= head_len) ? (rline_end + 2) : head_len;
    while (cur < head_len) {
        size_t le = cur;
        while (le + 1 < head_len && !(s[le] == '\r' && s[le + 1] == '\n')) le++;
        if (le + 1 >= head_len) le = head_len;
        const char *colon = (const char *)memchr(s + cur, ':', le - cur);
        if (colon) {
            size_t ks = cur, ke = (size_t)(colon - s);
            while (ks < ke && (s[ks] == ' ' || s[ks] == '\t')) ks++;
            while (ke > ks && (s[ke - 1] == ' ' || s[ke - 1] == '\t')) ke--;
            size_t vs = (size_t)(colon - s) + 1, ve = le;
            while (vs < ve && (s[vs] == ' ' || s[vs] == '\t')) vs++;
            while (ve > vs && (s[ve - 1] == ' ' || s[ve - 1] == '\t')) ve--;
            PbStr *kstr = vm_new_str(vm, s + ks, ke - ks);
            vm_temp_push(vm, val_obj(kstr));
            PbStr *vstr = vm_new_str(vm, s + vs, ve - vs);
            map_set(headers, kstr, val_obj(vstr));
            vm_temp_pop(vm, 1);
        }
        cur = (le + 2 <= head_len) ? (le + 2) : head_len;
    }

    PbMap *res = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "method"), val_obj(vm_new_str(vm, s + m0, m1 - m0)));
    map_set(res, vm_const_str(vm, "path"), val_obj(vm_new_str(vm, s + p0, p1 - p0)));
    map_set(res, vm_const_str(vm, "headers"), val_obj(headers));
    map_set(res, vm_const_str(vm, "body"), val_obj(vm_new_str(vm, body_ptr, body_len)));
    vm_temp_pop(vm, 2);
    *out = val_obj(res);
    return true;
}

static bool zr_build_http_resp(PbVM *vm, int status, const char *ct, const char *body, size_t body_len, PbValue *out) {
    ZrBuf b;
    zr_buf_init(&b);
    char head[512];
    snprintf(head, sizeof(head),
             "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n",
             status, zr_http_reason(status), ct, body_len);
    zr_buf_append_cstr(&b, head);
    zr_buf_append(&b, body, body_len);
    *out = val_obj(vm_new_str(vm, (const char *)b.data, b.len));
    zr_buf_free(&b);
    return true;
}

static bool bi_http_response(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "http_response(status, body) expects (number, string)");
        return false;
    }
    PbStr *body = (PbStr *)val_as_obj(argv[1]);
    return zr_build_http_resp(vm, (int)argv[0].u.num, "text/plain", body->chars, body->len, out);
}

static bool bi_http_response_ct(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "http_response_ct", "net")) return false;
    if (argc != 3 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "http_response_ct(status, content_type, body) expects (number, string, string)");
        return false;
    }
    PbStr *ct = (PbStr *)val_as_obj(argv[1]);
    PbStr *body = (PbStr *)val_as_obj(argv[2]);
    return zr_build_http_resp(vm, (int)argv[0].u.num, ct->chars, body->chars, body->len, out);
}

static bool bi_http_response_json(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "http_response_json", "net")) return false;
    if (argc != 2 || argv[0].tag != VAL_NUM) {
        vm_raise(vm, "http_response_json(status, value) expects (number, value)");
        return false;
    }
    ZrBuf jb;
    zr_buf_init(&jb);
    if (!zr_json_stringify_into(vm, argv[1], &jb, 0)) { zr_buf_free(&jb); return false; }
    bool ok = zr_build_http_resp(vm, (int)argv[0].u.num, "application/json",
                                 (const char *)(jb.data ? jb.data : ""), jb.len, out);
    zr_buf_free(&jb);
    return ok;
}

static bool bi_parse_sse_line(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "parse_sse_line() expects 1 string"); return false; }
    const char *line = ((PbStr *)val_as_obj(argv[0]))->chars;
    const char *field = "";
    const char *val = line;
    if (strncmp(line, "data: ", 6) == 0)        { field = "data";  val = line + 6; }
    else if (strncmp(line, "event: ", 7) == 0)  { field = "event"; val = line + 7; }
    else if (strncmp(line, "id: ", 4) == 0)     { field = "id";    val = line + 4; }
    else if (strncmp(line, "retry: ", 7) == 0)  { field = "retry"; val = line + 7; }

    PbMap *m = vm_new_map(vm, 4);
    vm_temp_push(vm, val_obj(m));
    map_set(m, vm_const_str(vm, "field"), val_obj(vm_new_cstr(vm, field)));
    map_set(m, vm_const_str(vm, "value"), val_obj(vm_new_cstr(vm, val)));
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

static bool bi_websocket_accept_key(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "websocket_accept_key() expects 1 string"); return false; }
    PbStr *key = (PbStr *)val_as_obj(argv[0]);
    const char *guid = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    ZrBuf b;
    zr_buf_init(&b);
    zr_buf_append(&b, key->chars, key->len);
    zr_buf_append_cstr(&b, guid);
    uint8_t sha[20];
    zr_sha1_oneshot(b.data, b.len, sha);
    zr_buf_free(&b);
    size_t elen = 0;
    char *enc = zr_base64_encode(sha, 20, &elen);
    *out = val_obj(vm_new_str(vm, enc, elen));
    free(enc);
    return true;
}

static bool bi_ws_parse_frame(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "ws_parse_frame() expects 1 string"); return false; }
    PbStr *frame = (PbStr *)val_as_obj(argv[0]);
    /* Decode UTF-8 codepoints (chars-as-bytes convention matching Rust VM) */
    uint32_t *codes = (uint32_t *)zr_xmalloc((frame->len + 1) * sizeof(uint32_t));
    size_t ncodes = 0, pos = 0;
    while (pos < frame->len) {
        size_t adv = 1;
        codes[ncodes++] = zr_utf8_decode(frame->chars + pos, frame->len - pos, &adv);
        pos += adv;
    }
    if (ncodes < 2) { free(codes); vm_raise(vm, "ws_parse_frame(): frame too short"); return false; }
    uint8_t b0 = (uint8_t)codes[0], b1 = (uint8_t)codes[1];
    double fin = (b0 & 0x80) ? 1.0 : 0.0;
    double opcode = (double)(b0 & 0x0f);
    bool masked = (b1 & 0x80) != 0;
    size_t len_code = (size_t)(b1 & 0x7f);
    size_t idx = 2, payload_len = 0;
    if (len_code <= 125) {
        payload_len = len_code;
    } else if (len_code == 126) {
        if (ncodes < idx + 2) { free(codes); vm_raise(vm, "ws_parse_frame(): short 16-bit length"); return false; }
        payload_len = ((size_t)codes[idx] << 8) | (size_t)codes[idx + 1];
        idx += 2;
    } else {
        if (ncodes < idx + 8) { free(codes); vm_raise(vm, "ws_parse_frame(): short 64-bit length"); return false; }
        uint64_t n64 = 0;
        for (int i = 0; i < 8; i++) n64 = (n64 << 8) | (uint64_t)codes[idx + i];
        idx += 8;
        payload_len = (size_t)n64;
    }
    uint8_t mask_key[4] = {0, 0, 0, 0};
    if (masked) {
        if (ncodes < idx + 4) { free(codes); vm_raise(vm, "ws_parse_frame(): missing mask key"); return false; }
        for (int i = 0; i < 4; i++) mask_key[i] = (uint8_t)codes[idx + i];
        idx += 4;
    }
    if (ncodes < idx + payload_len) { free(codes); vm_raise(vm, "ws_parse_frame(): truncated payload"); return false; }

    ZrBuf pb;
    zr_buf_init(&pb);
    for (size_t i = 0; i < payload_len; i++) {
        uint8_t b = (uint8_t)codes[idx + i];
        if (masked) b ^= mask_key[i & 3];
        uint8_t u8[4];
        size_t ulen = zr_utf8_encode((uint32_t)b, u8);
        zr_buf_append(&pb, u8, ulen);
    }
    free(codes);

    PbMap *m = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(m));
    map_set(m, vm_const_str(vm, "opcode"), val_num(opcode));
    map_set(m, vm_const_str(vm, "payload"), val_obj(vm_new_str(vm, (const char *)(pb.data ? pb.data : ""), pb.len)));
    map_set(m, vm_const_str(vm, "fin"), val_num(fin));
    map_set(m, vm_const_str(vm, "masked"), val_num(masked ? 1.0 : 0.0));
    map_set(m, vm_const_str(vm, "payload_len"), val_num((double)payload_len));
    map_set(m, vm_const_str(vm, "frame_len"), val_num((double)(idx + payload_len)));
    map_set(m, vm_const_str(vm, "next_index"), val_num((double)(idx + payload_len)));
    vm_temp_pop(vm, 1);
    zr_buf_free(&pb);
    *out = val_obj(m);
    return true;
}

static bool bi_ws_write_frame(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[0].tag != VAL_NUM || argv[0].u.num < 0.0 || argv[0].u.num > 15.0 || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "ws_write_frame(opcode, payload) expects (0..15, string)");
        return false;
    }
    uint8_t opcode = (uint8_t)argv[0].u.num;
    PbStr *payload = (PbStr *)val_as_obj(argv[1]);
    uint8_t *pbytes = (uint8_t *)zr_xmalloc(payload->len + 1);
    size_t plen = 0, pos = 0;
    while (pos < payload->len) {
        size_t adv = 1;
        uint32_t cp = zr_utf8_decode(payload->chars + pos, payload->len - pos, &adv);
        pos += adv;
        pbytes[plen++] = (uint8_t)(cp > 255 ? 255 : cp);
    }

    uint8_t mask_key[4] = {0x12, 0x34, 0x56, 0x78};
    zr_csprng_bytes(mask_key, 4);

    ZrBuf out_b;
    zr_buf_init(&out_b);
    uint8_t u8[4];
    zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)(0x80u | (opcode & 0x0fu)), u8));
    if (plen <= 125) {
        zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)(0x80u | (uint8_t)plen), u8));
    } else if (plen <= 65535) {
        zr_buf_append(&out_b, u8, zr_utf8_encode(0x80u | 126u, u8));
        zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)((plen >> 8) & 0xffu), u8));
        zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)(plen & 0xffu), u8));
    } else {
        zr_buf_append(&out_b, u8, zr_utf8_encode(0x80u | 127u, u8));
        uint64_t p64 = (uint64_t)plen;
        for (int sh = 56; sh >= 0; sh -= 8) {
            zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)((p64 >> sh) & 0xffu), u8));
        }
    }
    for (int i = 0; i < 4; i++) zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)mask_key[i], u8));
    for (size_t i = 0; i < plen; i++) {
        zr_buf_append(&out_b, u8, zr_utf8_encode((uint32_t)(pbytes[i] ^ mask_key[i & 3]), u8));
    }
    free(pbytes);
    *out = val_obj(vm_new_str(vm, (const char *)(out_b.data ? out_b.data : ""), out_b.len));
    zr_buf_free(&out_b);
    return true;
}

/* ---- Thin AF_UNIX Domain Socket OS Syscall Wrappers (#12) ---- */
#if !defined(_WIN32) && !defined(__wasi__)
#include <sys/un.h>
#endif
#if defined(_WIN32)
typedef struct { unsigned short sun_family; char sun_path[108]; } ZrWinSockAddrUn;
#endif

static bool bi_unix_listen(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "unix_listen", "net")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "unix_listen(path) expects 1 string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
#if defined(__wasi__)
    (void)path; (void)out;
    vm_raise(vm, "unix_listen(): unavailable in WASI sandbox");
    return false;
#elif defined(_WIN32)
    zr_net_ensure_init();
    ZrWinSockAddrUn su;
    memset(&su, 0, sizeof(su));
    su.sun_family = 1; /* AF_UNIX */
    if (strlen(path) >= sizeof(su.sun_path)) { vm_raise(vm, "unix_listen(): socket path too long"); return false; }
    memcpy(su.sun_path, path, strlen(path));
    wchar_t *wp = zr_win_utf8_to_wide(path, true);
    if (wp) { DeleteFileW(wp); free(wp); }
    ZrSock s = socket(1, SOCK_STREAM, 0);
    if (s == ZR_INVALID_SOCK || bind(s, (const struct sockaddr *)&su, (int)sizeof(su)) != 0 || listen(s, 64) != 0) {
        if (s != ZR_INVALID_SOCK) zr_close_raw_sock(s);
        vm_raise(vm, "unix_listen('%s'): AF_UNIX bind/listen failed", path);
        return false;
    }
    size_t id = zr_net_insert(ZR_NET_LISTENER, s, NULL, NULL);
    *out = val_num((double)id);
    return true;
#else
    struct sockaddr_un su;
    memset(&su, 0, sizeof(su));
    su.sun_family = AF_UNIX;
    if (strlen(path) >= sizeof(su.sun_path)) { vm_raise(vm, "unix_listen(): socket path too long"); return false; }
    memcpy(su.sun_path, path, strlen(path));
    if (path[0] != '@') (void)unlink(path);
    else su.sun_path[0] = '\0'; /* Linux abstract namespace */
    ZrSock s = socket(AF_UNIX, SOCK_STREAM, 0);
    socklen_t slen = (path[0] == '@') ? (socklen_t)(offsetof(struct sockaddr_un, sun_path) + strlen(path)) : (socklen_t)sizeof(su);
    if (s == ZR_INVALID_SOCK || bind(s, (const struct sockaddr *)&su, slen) != 0 || listen(s, 64) != 0) {
        if (s != ZR_INVALID_SOCK) zr_close_raw_sock(s);
        vm_raise(vm, "unix_listen('%s'): %s", path, strerror(errno));
        return false;
    }
    size_t id = zr_net_insert(ZR_NET_LISTENER, s, NULL, NULL);
    *out = val_num((double)id);
    return true;
#endif
}

static bool bi_unix_connect(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_NET), "unix_connect", "net")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "unix_connect(path) expects 1 string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
#if defined(__wasi__)
    (void)path; (void)out;
    vm_raise(vm, "unix_connect(): unavailable in WASI sandbox");
    return false;
#elif defined(_WIN32)
    zr_net_ensure_init();
    ZrWinSockAddrUn su;
    memset(&su, 0, sizeof(su));
    su.sun_family = 1;
    if (strlen(path) >= sizeof(su.sun_path)) { vm_raise(vm, "unix_connect(): socket path too long"); return false; }
    memcpy(su.sun_path, path, strlen(path));
    ZrSock s = socket(1, SOCK_STREAM, 0);
    if (s == ZR_INVALID_SOCK || connect(s, (const struct sockaddr *)&su, (int)sizeof(su)) != 0) {
        if (s != ZR_INVALID_SOCK) zr_close_raw_sock(s);
        vm_raise(vm, "unix_connect('%s'): connect failed", path);
        return false;
    }
    size_t id = zr_net_insert(ZR_NET_STREAM, s, NULL, NULL);
    *out = val_num((double)id);
    return true;
#else
    struct sockaddr_un su;
    memset(&su, 0, sizeof(su));
    su.sun_family = AF_UNIX;
    if (strlen(path) >= sizeof(su.sun_path)) { vm_raise(vm, "unix_connect(): socket path too long"); return false; }
    memcpy(su.sun_path, path, strlen(path));
    if (path[0] == '@') su.sun_path[0] = '\0';
    ZrSock s = socket(AF_UNIX, SOCK_STREAM, 0);
    socklen_t slen = (path[0] == '@') ? (socklen_t)(offsetof(struct sockaddr_un, sun_path) + strlen(path)) : (socklen_t)sizeof(su);
    if (s == ZR_INVALID_SOCK || connect(s, (const struct sockaddr *)&su, slen) != 0) {
        if (s != ZR_INVALID_SOCK) zr_close_raw_sock(s);
        vm_raise(vm, "unix_connect('%s'): %s", path, strerror(errno));
        return false;
    }
    size_t id = zr_net_insert(ZR_NET_STREAM, s, NULL, NULL);
    *out = val_num((double)id);
    return true;
#endif
}

static bool bi_unix_peer_cred(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || (argv[0].tag != VAL_NUM && argv[0].tag != VAL_I64)) {
        vm_raise(vm, "unix_peer_cred(sock_id) expects 1 handle");
        return false;
    }
    size_t id = (argv[0].tag == VAL_I64) ? (size_t)argv[0].as.i64 : (size_t)argv[0].u.num;
    ZrNetEntry *e = zr_net_checkout(id);
    if (!e) { vm_raise(vm, "unix_peer_cred(): invalid socket handle"); return false; }
    PbMap *m = pb_map_new_raw(vm, 8);
    vm_temp_push(vm, val_obj(m));
#if defined(__linux__) && defined(SO_PEERCRED)
    struct { int32_t pid; uint32_t uid; uint32_t gid; } cr = {0, 0, 0};
    socklen_t clen = (socklen_t)sizeof(cr);
    bool ok = (getsockopt(e->sock, SOL_SOCKET, SO_PEERCRED, &cr, &clen) == 0);
    pb_map_set_cstr(vm, m, "ok", val_bool(ok));
    pb_map_set_cstr(vm, m, "pid", val_i64((int64_t)cr.pid));
    pb_map_set_cstr(vm, m, "uid", val_i64((int64_t)cr.uid));
    pb_map_set_cstr(vm, m, "gid", val_i64((int64_t)cr.gid));
#else
    pb_map_set_cstr(vm, m, "ok", val_bool(false));
#endif
    zr_net_checkin(e);
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

#endif /* PBLANG_ZR_NET_HTTP_TLS_H */
