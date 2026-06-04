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
