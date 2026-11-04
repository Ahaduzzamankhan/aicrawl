# AICrawl request schema (v1)

AICrawl is JSON-first. A request describes **what** to extract with structured
selectors; the engine fetches, parses, selects, transforms, validates and stores
the result. No model, prompt or API key is involved.

* Request version: `1` (`version` is optional and must equal `"1"` when present).
* Response envelope is identical for the CLI, REST API and MCP tools.
* The same request object works everywhere; only the transport changes.

## Request object

```json
{
  "version": "1",
  "url": "https://example.com/products",
  "action": "extract",
  "render": "auto",
  "use_cache": true,
  "selectors": {
    "products": {
      "type": "list",
      "selector": ".product-card",
      "fields": {
        "name": { "selector": ".product-title", "type": "text" },
        "price": { "selector": ".price", "type": "number" },
        "image": { "selector": "img", "attribute": "src" },
        "link": { "selector": "a", "attribute": "href", "type": "url" }
      }
    }
  },
  "pagination": { "enabled": true, "next_selector": "a.next", "max_pages": 10 },
  "deduplicate": { "enabled": true, "keys": ["name"] },
  "output": { "format": "json", "pretty": true }
}
```

### Top-level fields

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `version` | string | `"1"` | Contract version. Any other value is rejected. |
| `url` | string | – | Single target URL (required for `extract`, `crawl`, `map`). |
| `urls` | string[] | `[]` | Multiple targets (`action: "batch"`). Duplicates are removed. |
| `action` | `extract` \| `batch` \| `crawl` \| `map` | `extract` | Operation. |
| `render` | `auto` \| `off` \| `on` | `auto` | `auto` renders only when the page looks JavaScript-driven. |
| `use_cache` | bool | `true` | Reuse the SQLite page cache. |
| `selectors` | object | `{}` | Named rules. Empty means "document summary". |
| `pagination` | object | disabled | Follow a next link while extracting. |
| `deduplicate` | object | disabled | Remove duplicate items after merging pages. |
| `schema` | object | – | JSON Schema the extracted data must validate against. |
| `include_document` | bool | `false` | Add a `_document` summary alongside rule output. |
| `crawl` | object | – | Crawl limits (see below). |
| `output` | object | `{json, pretty}` | Serialization preferences. |

### Rule object

| Field | Type | Meaning |
| --- | --- | --- |
| `type` | `text` \| `html` \| `attribute` \| `url` \| `number` \| `integer` \| `boolean` \| `list` \| `object` \| `json_ld` \| `meta` | Conversion applied to the match. |
| `selector` | string | CSS selector evaluated inside the parent scope. |
| `xpath` | string | XPath expression (see the supported subset below). |
| `attribute` | string | Attribute for `attribute`/`url` rules (`href` by default for `url`). |
| `name` | string | Meta name/property for `meta` rules. |
| `fields` | object | Nested rules; makes the rule an object (or a list of objects). |
| `required` | bool | Fail the request when the value is missing. |
| `default` | any | Value used when an optional field is missing. |
| `multiple` | bool | Collect every match instead of the first. |
| `max_items` | number | Cap on items produced by a list rule. |
| `transform` | string \| string[] | `trim`, `collapse_whitespace`, `lowercase`, `uppercase`, `strip_html`, `number`, `integer`, `boolean`, `absolute_url`. |
| `trim` | bool | Override whitespace handling for text values. |
| `absolute_url` | bool | Resolve values against the page URL. |

`selector` and `xpath` are mutually exclusive.

### Pagination

| Field | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | Follow the next link. |
| `next_selector` | – | CSS selector or XPath (`/...`, `//...`) for the next link. |
| `max_pages` | `10` | Clamped to 1–100. |
| `same_origin` | `true` | Refuse next links on another origin. |

### Dedupe

`{"enabled": true, "keys": ["name", "price"]}` removes later items whose key
values match an earlier item. An empty key list disables dedupe.

### Crawl

| Field | Default | Meaning |
| --- | --- | --- |
| `max_pages` | `10` | Clamped to 1–500. |
| `max_depth` | `2` | Clamped to 0–5. |
| `same_domain` | `true` | Restrict to the starting host. |
| `incremental` | `false` | Compare content hashes with the previous crawl. |
| `sitemap` | `true` | Discover URLs from sitemap.xml. |

## Response envelope

Success:

```json
{
  "success": true,
  "data": { "products": [] },
  "meta": {
    "url": "https://example.com/products",
    "final_url": "https://example.com/products",
    "status": 200,
    "pages_processed": 1,
    "items_extracted": 0,
    "job_id": "job_…",
    "result_id": "res_…",
    "duration_ms": 120,
    "render": "http",
    "cached": false,
    "truncated": false,
    "projection": "rules"
  },
  "errors": []
}
```

* `errors` lists field-level issues. Optional misses produce
  `SELECTOR_NOT_FOUND` with `required: false` and `success: true`.
* `meta.pages` is present for crawl and batch responses.

Failure:

```json
{
  "success": false,
  "data": null,
  "error": {
    "code": "FIELD_REQUIRED",
    "message": "1 required field(s) were not found: products.name",
    "retryable": false
  },
  "errors": [
    {
      "code": "SELECTOR_NOT_FOUND",
      "field": "products.name",
      "message": "no value matched for field 'products.name'",
      "required": true
    }
  ]
}
```

### Error codes

| Code | HTTP | Meaning |
| --- | --- | --- |
| `INVALID_REQUEST` | 400 | Malformed JSON, bad selector, unsupported version, missing URL. |
| `FIELD_REQUIRED` | 400/422 | A required field produced no value. |
| `SCHEMA_VALIDATION_FAILED` | 422 | Extracted data violates the supplied JSON Schema. |
| `BLOCKED_BY_POLICY` | 403 | SSRF/domain/scheme policy refusal. |
| `ROBOTS_DISALLOWED` | 403 | robots.txt forbids the path. |
| `NOT_FOUND` | 404 | Unknown job or result. |
| `RATE_LIMITED` | 429 | Per-host request budget exhausted (retryable). |
| `TIMEOUT` | 504 | Deadline exceeded (retryable). |
| `NETWORK_ERROR` / `HTTP_ERROR` | 502 | Transport or upstream failure. |
| `RENDER_UNAVAILABLE` | 503 | Playwright requested but not installed. |
| `BODY_TOO_LARGE` | 413 | Response exceeded the configured cap. |
| `UNSUPPORTED_CONTENT` | 502 | Body is not HTML, XML, JSON or text. |
| `DATABASE_ERROR` / `IO_ERROR` / `CONFIG_ERROR` | 500 | Local failures. |

AICrawl never fabricates values: a missing optional field becomes `null` (or its
`default`) and is reported in `errors`; a missing required field fails the
request.

## Document projection

When `selectors` is empty the engine returns a structural summary:

```json
{
  "document": {
    "url": "https://example.com/products",
    "title": "Products",
    "description": "…",
    "lang": "en",
    "headings": [{ "level": 1, "text": "Products" }],
    "open_graph": { "og:title": "Products" },
    "meta": { "description": "…" },
    "json_ld": [{ "@type": "Product", "name": "…" }],
    "tables": [{ "headers": ["Name"], "rows": [["Widget"]] }],
    "links": [{ "url": "https://example.com/p/1", "text": "Details", "rel": null }],
    "images": ["https://example.com/a.png"],
    "prices": ["$19.99"],
    "word_count": 412,
    "text": "…"
  }
}
```

Add `"include_document": true` to keep this summary alongside rule output.

## XPath subset

AICrawl evaluates XPath over the same DOM as CSS selectors. Supported syntax:

* paths: `/html/body/div`, `//div`, `.//span`
* wildcard: `*`
* attributes: `//a/@href`
* text nodes: `//h1/text()`
* positional predicates: `[1]`, `[last()]`
* attribute predicates: `[@class]`, `[@class='card']`
* substring predicates: `[contains(@class,'card')]`

Axes, unions, functions other than `contains`, and arithmetic are rejected with
`INVALID_REQUEST` rather than silently ignored.

## Structured data

`json_ld` rules return every `application/ld+json` block (or those nested inside
the matched node). `meta` rules read `<meta name|property>` values. The document
summary exposes Open Graph keys under `open_graph` and all meta tags under
`meta`. Relative URLs are resolved against the final page URL.

## Interfaces

| Interface | Entry point |
| --- | --- |
| CLI | `aicrawl extract --input request.json` |
| REST | `POST /api/v1/extract` with the request object as the body |
| MCP | `extract_structured_data` with `{"request": {…}}` |
| Dashboard | the JSON request editor on `/extract` |

See `examples/` for runnable requests.
