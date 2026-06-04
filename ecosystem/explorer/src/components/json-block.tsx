export function JsonBlock({ value }: { value: unknown }) {
  return (
    <pre className="max-h-[28rem] overflow-auto rounded-md border border-white/10 bg-black/40 p-4 text-xs leading-5 text-zinc-200">
      {JSON.stringify(value, null, 2)}
    </pre>
  );
}
