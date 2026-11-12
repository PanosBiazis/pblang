/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v2.0.0 — zr_net_http_tls.h
 * Thread-safe TCP sockets, lazy-dlopen OpenSSL TLS 1.3 (Linux) / WinHTTP (Windows),
 * HTTP/1.1 client + server helpers, SSE parser, and RFC 6455 WebSocket codecs.
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
static void zr_ensure_winsock(void) {
    if (!g_zr_wsa_ready) {
        WSADATA wd;
        if (WSAStartup(MAKEWORD(2, 2), &wd) == 0) g_zr_wsa_ready = true;
    }
}
#else
typedef int ZrSock;
#define ZR_INVALID_SOCK (-1)
static void zr_close_raw_sock(ZrSock s) { if (s >= 0) close(s); }
static void zr_ensure_winsock(void) {}
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
    ZR_NET_STREAM
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_listen", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_connect", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_accept", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_read", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_read_until", "net")) return false;
    if (argc != 3 || !val_is_obj(argv[1], OBJ_STR) || argv[2].tag != VAL_NUM || argv[2].u.num < 0.0) {
        vm_raise(vm, "tcp_read_until(handle, delim, max_bytes) expects (handle, string, number)");
        return false;
    }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_read_until", &id)) return false;
    PbStr *delim = (PbStr *)val_as_obj(argv[1]);
    size_t max_bytes = (size_t)argv[2].u.num;

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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_write", "net")) return false;
    if (argc != 2) { vm_raise(vm, "tcp_write() expects 2 arguments"); return false; }
    size_t id = 0;
    if (!zr_net_arg_handle(vm, argv[0], "tcp_write", &id)) return false;

    uint8_t *alloc_bytes = NULL;
    const uint8_t *ptr = NULL;
    size_t len = 0;
    if (val_is_obj(argv[1], OBJ_STR)) {
        PbStr *s = (PbStr *)val_as_obj(argv[1]);
        ptr = (const uint8_t *)s->chars;
        len = s->len;
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
        vm_raise(vm, "tcp_write(): second argument must be string or byte array");
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_close", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_local_addr", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_listen_tls", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_connect_tls", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "tcp_wrap_tls", "net")) return false;
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

static bool zr_decode_chunked_body(const uint8_t *in, size_t in_len, ZrBuf *out) {
    size_t pos = 0;
    while (pos < in_len) {
        /* Find \r\n ending chunk size line */
        size_t line_end = pos;
        while (line_end + 1 < in_len && !(in[line_end] == '\r' && in[line_end + 1] == '\n')) line_end++;
        if (line_end + 1 >= in_len) break;
        size_t chunk_sz = 0;
        for (size_t i = pos; i < line_end; i++) {
            if (in[i] == ';') break;
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

static bool zr_http_request_exec(PbVM *vm, const char *method, const char *url,
                                 const char **hdr_keys, const char **hdr_vals, size_t nhdrs,
                                 const uint8_t *body, size_t body_len, ZrBuf *resp_body, int redirect_depth) {
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

#if defined(_WIN32)
    /* Native WinHTTP path on Windows (handles system TLS 1.3 + OS root store automatically) */
    wchar_t whost[512], wpath[2048], wmethod[16];
    MultiByteToWideChar(CP_UTF8, 0, host, -1, whost, 512);
    MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, 2048);
    MultiByteToWideChar(CP_UTF8, 0, method, -1, wmethod, 16);

    HINTERNET hSession = WinHttpOpen(L"pblang-zr/2.0", WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                     WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) { vm_raise(vm, "WinHttpOpen failed"); return false; }
    int tms = (int)(vm->caps.io_timeout_ms ? vm->caps.io_timeout_ms : 30000);
    WinHttpSetTimeouts(hSession, tms, tms, tms, tms);

    HINTERNET hConnect = WinHttpConnect(hSession, whost, (INTERNET_PORT)port, 0);
    if (!hConnect) { WinHttpCloseHandle(hSession); vm_raise(vm, "WinHttpConnect failed"); return false; }

    DWORD flags = is_https ? WINHTTP_FLAG_SECURE : 0;
    HINTERNET hReq = WinHttpOpenRequest(hConnect, wmethod, wpath, NULL, WINHTTP_NO_REFERER,
                                        WINHTTP_DEFAULT_ACCEPT_TYPES, flags);
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
    DWORD avail = 0;
    while (WinHttpQueryDataAvailable(hReq, &avail) && avail > 0) {
        uint8_t *tmp = (uint8_t *)zr_xmalloc(avail);
        DWORD got = 0;
        if (WinHttpReadData(hReq, tmp, avail, &got) && got > 0) zr_buf_append(resp_body, tmp, got);
        free(tmp);
    }
    WinHttpCloseHandle(hReq);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);
    return true;
#else
    char port_s[16], err[256];
    snprintf(port_s, sizeof(port_s), "%u", (unsigned)port);
    ZrSock s = zr_connect_tcp_raw(host, port_s, vm->caps.io_timeout_ms, err);
    if (s == ZR_INVALID_SOCK) { vm_raise(vm, "http %s '%s': %s", method, url, err); return false; }

    ZrNetEntry tmp_e;
    memset(&tmp_e, 0, sizeof(tmp_e));
    tmp_e.kind = ZR_NET_STREAM;
    tmp_e.sock = s;
    if (is_https) {
        if (!zr_tls_client_wrap(s, host, &tmp_e.ssl_ctx, &tmp_e.ssl, err)) {
            zr_close_raw_sock(s);
            vm_raise(vm, "https %s '%s': %s", method, url, err);
            return false;
        }
    }

    ZrBuf req;
    zr_buf_init(&req);
    char head_top[3072];
    if ((is_https && port == 443) || (!is_https && port == 80)) {
        snprintf(head_top, sizeof(head_top), "%s %s HTTP/1.1\r\nHost: %s\r\nUser-Agent: pblang/2.0\r\nConnection: close\r\n",
                 method, path, host);
    } else {
        snprintf(head_top, sizeof(head_top), "%s %s HTTP/1.1\r\nHost: %s:%u\r\nUser-Agent: pblang/2.0\r\nConnection: close\r\n",
                 method, path, host, (unsigned)port);
    }
    zr_buf_append_cstr(&req, head_top);
    for (size_t i = 0; i < nhdrs; i++) {
        zr_buf_append_cstr(&req, hdr_keys[i]);
        zr_buf_append_cstr(&req, ": ");
        zr_buf_append_cstr(&req, hdr_vals[i]);
        zr_buf_append_cstr(&req, "\r\n");
    }
    if (body_len > 0 || strcmp(method, "POST") == 0) {
        char cl[64];
        snprintf(cl, sizeof(cl), "Content-Length: %zu\r\n", body_len);
        zr_buf_append_cstr(&req, cl);
    }
    zr_buf_append_cstr(&req, "\r\n");
    if (body_len > 0) zr_buf_append(&req, body, body_len);

    if (zr_stream_write_all(&tmp_e, (const uint8_t *)req.data, req.len, err) < 0) {
        zr_buf_free(&req);
        zr_net_destroy_entry(&tmp_e);
        vm_raise(vm, "http write '%s': %s", url, err);
        return false;
    }
    zr_buf_free(&req);

    ZrBuf raw;
    zr_buf_init(&raw);
    uint8_t rbuf[8192];
    for (;;) {
        ssize_t n = zr_stream_read_some(&tmp_e, rbuf, sizeof(rbuf), err);
        if (n <= 0) break;
        zr_buf_append(&raw, rbuf, (size_t)n);
        if (raw.len > 64 * 1024 * 1024) break;
    }
    zr_net_destroy_entry(&tmp_e);

    ptrdiff_t sep = zr_find_bytes((const uint8_t *)raw.data, raw.len, (const uint8_t *)"\r\n\r\n", 4);
    if (sep < 0) {
        zr_buf_free(&raw);
        vm_raise(vm, "http '%s': malformed HTTP response", url);
        return false;
    }

    char *hdr_str = (char *)zr_xmalloc((size_t)sep + 1);
    memcpy(hdr_str, raw.data, (size_t)sep);
    hdr_str[sep] = '\0';

    int status_code = 0;
    sscanf(hdr_str, "HTTP/%*s %d", &status_code);

    /* Check Redirect 301/302/303/307/308 */
    if (status_code == 301 || status_code == 302 || status_code == 303 || status_code == 307 || status_code == 308) {
        char *loc = NULL;
        for (char *p = hdr_str; *p; p++) {
            if ((p == hdr_str || p[-1] == '\n') && strncasecmp(p, "location:", 9) == 0) {
                loc = p + 9;
                while (*loc == ' ' || *loc == '\t') loc++;
                char *eol = strpbrk(loc, "\r\n");
                if (eol) *eol = '\0';
                break;
            }
        }
        if (loc && loc[0]) {
            char next_url[2560];
            if (strncmp(loc, "http://", 7) == 0 || strncmp(loc, "https://", 8) == 0) {
                snprintf(next_url, sizeof(next_url), "%s", loc);
            } else {
                snprintf(next_url, sizeof(next_url), "%s://%s:%u%s%s",
                         is_https ? "https" : "http", host, (unsigned)port,
                         loc[0] == '/' ? "" : "/", loc);
            }
            free(hdr_str);
            zr_buf_free(&raw);
            return zr_http_request_exec(vm, (status_code == 303) ? "GET" : method, next_url,
                                        hdr_keys, hdr_vals, nhdrs,
                                        (status_code == 303) ? NULL : body,
                                        (status_code == 303) ? 0 : body_len,
                                        resp_body, redirect_depth + 1);
        }
    }

    bool is_chunked = false;
    for (char *p = hdr_str; *p; p++) {
        if ((p == hdr_str || p[-1] == '\n') && strncasecmp(p, "transfer-encoding:", 18) == 0) {
            if (strstr(p + 18, "chunked") || strstr(p + 18, "CHUNKED")) is_chunked = true;
        }
    }
    free(hdr_str);

    const uint8_t *bstart = (const uint8_t *)(raw.data + sep + 4);
    size_t blen = raw.len - (size_t)sep - 4;
    if (status_code >= 400) {
        zr_buf_free(&raw);
        vm_raise(vm, "http status %d for '%s'", status_code, url);
        return false;
    }
    if (is_chunked) zr_decode_chunked_body(bstart, blen, resp_body);
    else zr_buf_append(resp_body, bstart, blen);
    zr_buf_free(&raw);
    return true;
#endif
}

static bool bi_http_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_http, "http_get", "http")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "http_get() expects 1 URL string"); return false; }
    ZrBuf resp;
    zr_buf_init(&resp);
    if (!zr_http_request_exec(vm, "GET", ((PbStr *)val_as_obj(argv[0]))->chars, NULL, NULL, 0, NULL, 0, &resp, 0)) {
        zr_buf_free(&resp);
        return false;
    }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_post(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_http, "http_post", "http")) return false;
    if ((argc != 2 && argc != 3) || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "http_post(url, body[, content_type]) expects strings");
        return false;
    }
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
    if (!zr_http_request_exec(vm, "POST", ((PbStr *)val_as_obj(argv[0]))->chars,
                              hk, hv, 1, (const uint8_t *)body->chars, body->len, &resp, 0)) {
        zr_buf_free(&resp);
        return false;
    }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_get_auth(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_http, "http_get_auth", "http")) return false;
    if (argc < 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "http_get_auth(url, token) expects 2 strings");
        return false;
    }
    PbStr *tok = (PbStr *)val_as_obj(argv[1]);
    char *bearer = (char *)zr_xmalloc(tok->len + 16);
    snprintf(bearer, tok->len + 16, "Bearer %s", tok->chars);
    const char *hk[1] = { "Authorization" };
    const char *hv[1] = { bearer };
    ZrBuf resp;
    zr_buf_init(&resp);
    bool ok = zr_http_request_exec(vm, "GET", ((PbStr *)val_as_obj(argv[0]))->chars, hk, hv, 1, NULL, 0, &resp, 0);
    free(bearer);
    if (!ok) { zr_buf_free(&resp); return false; }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
    return true;
}

static bool bi_http_post_auth(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_http, "http_post_auth", "http")) return false;
    if (argc < 3 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "http_post_auth(url, headers_map, body) expects (string, map, string)");
        return false;
    }
    const char **hk = NULL;
    const char **hv = NULL;
    size_t nh = 0;
    if (val_is_obj(argv[1], OBJ_MAP)) {
        PbMap *m = (PbMap *)val_as_obj(argv[1]);
        hk = (const char **)zr_xmalloc((m->count + 1) * sizeof(char *));
        hv = (const char **)zr_xmalloc((m->count + 1) * sizeof(char *));
        for (size_t i = 0; i < m->used; i++) {
            if (m->entries[i].active && m->entries[i].key && val_is_obj(m->entries[i].val, OBJ_STR)) {
                hk[nh] = m->entries[i].key->chars;
                hv[nh] = ((PbStr *)val_as_obj(m->entries[i].val))->chars;
                nh++;
            }
        }
    }
    PbStr *body = (PbStr *)val_as_obj(argv[2]);
    ZrBuf resp;
    zr_buf_init(&resp);
    bool ok = zr_http_request_exec(vm, "POST", ((PbStr *)val_as_obj(argv[0]))->chars,
                                   hk, hv, nh, (const uint8_t *)body->chars, body->len, &resp, 0);
    free(hk);
    free(hv);
    if (!ok) { zr_buf_free(&resp); return false; }
    *out = val_obj(vm_new_str(vm, (const char *)(resp.data ? resp.data : ""), resp.len));
    zr_buf_free(&resp);
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "http_parse_request", "net")) return false;
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
    if (!vm_check_cap(vm, vm->caps.allow_net, "http_response_ct", "net")) return false;
    if (argc != 3 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR) || !val_is_obj(argv[2], OBJ_STR)) {
        vm_raise(vm, "http_response_ct(status, content_type, body) expects (number, string, string)");
        return false;
    }
    PbStr *ct = (PbStr *)val_as_obj(argv[1]);
    PbStr *body = (PbStr *)val_as_obj(argv[2]);
    return zr_build_http_resp(vm, (int)argv[0].u.num, ct->chars, body->chars, body->len, out);
}

static bool bi_http_response_json(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_net, "http_response_json", "net")) return false;
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

#endif /* PBLANG_ZR_NET_HTTP_TLS_H */
