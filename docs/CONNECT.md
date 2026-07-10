# Connecting the console to a live IPFRS node

The console is served over **https** (GitHub Pages), so it can only talk to an
**https** gateway — a browser blocks https→http requests (mixed content). Run the
gateway with TLS and CORS allowing the console's origin.

## 1. A TLS certificate

For a public node use a real certificate (Let's Encrypt, etc.). For local testing,
a self-signed one works (the browser will ask you to trust it once):

```bash
openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
```

## 2. Run the gateway with knowledge, TLS and CORS

Set `IPFRS_CORS_ORIGINS` to the console's origin (e.g. the Pages URL, or
`http://localhost:5273` for local dev), and point TLS at the cert/key:

```bash
IPFRS_CORS_ORIGINS='https://<user>.github.io' \
IPFRS_TLS_CERT=cert.pem IPFRS_TLS_KEY=key.pem \
  cargo run --example simple_server -p ipfrs-interface -- dev
```

or with the CLI:

```bash
ipfrs gateway --listen 0.0.0.0:8080 --tls-cert cert.pem --tls-key key.pem
# (set IPFRS_CORS_ORIGINS in the environment)
```

The gateway now serves `/api/v0/knowledge/*` over https with the right CORS
headers. Verify:

```bash
curl -sk https://127.0.0.1:8080/api/v0/knowledge/stats
# {"entities":0,"index":0}
```

## 3. Point the console at it

Open the console → **Settings** (gear):

1. Storage mode → **Live · IPFRS gateway**
2. Gateway address → `https://<your-node>:8080`
3. **Test connection** → should go green.

Now open **🧠 Поиск по знаниям**. In live mode the panel seeds the demo graph on
the node (if empty), commits a real head CID, and every search / projection runs
against `/api/v0/knowledge/*`. Uploads, DAG explorer, and the rest of the console
use the same live gateway.

## Knowledge API surface

| Method & path | Purpose |
|---|---|
| `POST /api/v0/knowledge/entity` | add/replace an entity |
| `POST /api/v0/knowledge/relation` | add a relation |
| `POST /api/v0/knowledge/commit` | persist a head (survives restart) |
| `POST /api/v0/knowledge/search` | cosine top-k over the vector index |
| `GET  /api/v0/knowledge/stats` \| `projection` | counts \| Markdown pages |
| `POST /api/v0/knowledge/pin` \| `unpin`, `GET pins` | GC pin set of heads |
| `POST /api/v0/knowledge/gc` | mark-and-sweep the cold tier |
| `GET  /api/v0/knowledge/export` \| `POST import` | whole graph as one CAR file |
