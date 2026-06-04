export class EntropisApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly statusText: string,
    public readonly responseText: string,
    public readonly publicMessage: string,
  ) {
    super(`Entropis API error ${status}: ${publicMessage}`);
    this.name = "EntropisApiError";
  }
}

export async function apiError(response: Response): Promise<EntropisApiError> {
  return apiErrorFromText(response, await response.text());
}

export function apiErrorFromText(response: Response, text: string): EntropisApiError {
  let publicMessage = text || response.statusText || "request failed";
  try {
    const parsed = JSON.parse(text) as { error?: unknown };
    if (typeof parsed.error === "string" && parsed.error.length > 0) {
      publicMessage = parsed.error;
    }
  } catch {
    // Keep non-JSON provider or proxy text as the public message.
  }
  return new EntropisApiError(response.status, response.statusText, text, publicMessage);
}
