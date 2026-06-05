import type { IncomingMessage, ServerResponse } from "node:http"
import { readFile } from "node:fs/promises"
import { extname, join, normalize } from "node:path"

const MAX_JSON_BYTES = 16 * 1024

export async function readJson(request: IncomingMessage) {
  const chunks: Buffer[] = []
  let size = 0

  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
    size += buffer.length
    if (size > MAX_JSON_BYTES) {
      throw new Error("request_too_large")
    }
    chunks.push(buffer)
  }

  const text = Buffer.concat(chunks).toString("utf8")
  return text ? (JSON.parse(text) as unknown) : {}
}

export function json(
  response: ServerResponse,
  status: number,
  body: unknown,
  headers: Record<string, string> = {},
) {
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "cache-control": "no-store",
    ...headers,
  })
  response.end(JSON.stringify(body))
}

export function redirect(
  response: ServerResponse,
  location: string,
  headers: Record<string, string> = {},
) {
  response.writeHead(302, { location, ...headers })
  response.end()
}

export function methodNotAllowed(response: ServerResponse) {
  json(response, 405, { error: "method_not_allowed" })
}

export function cookieValue(request: IncomingMessage, name: string) {
  const cookie = request.headers.cookie
  if (!cookie) return null

  for (const part of cookie.split(";")) {
    const [key, value] = part.trim().split("=")
    if (key === name && value) {
      return decodeURIComponent(value)
    }
  }

  return null
}

export function setCookie(name: string, value: string, maxAgeSeconds: number, secure: boolean) {
  const secureFlag = secure ? "; Secure" : ""
  return `${name}=${encodeURIComponent(value)}; HttpOnly; Path=/; SameSite=Lax; Max-Age=${maxAgeSeconds}${secureFlag}`
}

export function clearCookie(name: string) {
  return `${name}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0`
}

export function requestOrigin(request: IncomingMessage) {
  const proto = request.headers["x-forwarded-proto"] ?? "http"
  const host = request.headers["x-forwarded-host"] ?? request.headers.host ?? "127.0.0.1:3002"
  return `${Array.isArray(proto) ? proto[0] : proto}://${Array.isArray(host) ? host[0] : host}`
}

export function requestIsSecure(request: IncomingMessage) {
  return requestOrigin(request).startsWith("https://")
}

export function clientIp(request: IncomingMessage) {
  const forwarded = request.headers["x-forwarded-for"]
  if (typeof forwarded === "string" && forwarded.length > 0) {
    return forwarded.split(",")[0]?.trim() ?? "unknown"
  }

  return request.socket.remoteAddress ?? "unknown"
}

export async function serveStatic(response: ServerResponse, pathname: string) {
  const safePath = normalize(pathname === "/" ? "/index.html" : pathname)
    .replace(/^(\.\.[/\\])+/, "")
    .replace(/^[/\\]+/, "")
  const filePath = join(process.cwd(), "dist", safePath)

  try {
    const bytes = await readFile(filePath)
    response.writeHead(200, { "content-type": mimeType(filePath) })
    response.end(bytes)
    return true
  } catch {
    return false
  }
}

function mimeType(path: string) {
  switch (extname(path)) {
    case ".html":
      return "text/html; charset=utf-8"
    case ".js":
      return "text/javascript; charset=utf-8"
    case ".css":
      return "text/css; charset=utf-8"
    case ".png":
      return "image/png"
    case ".svg":
      return "image/svg+xml"
    default:
      return "application/octet-stream"
  }
}
