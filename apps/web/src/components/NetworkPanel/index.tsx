import type { NetworkEntry } from "@roku-emu/protocol";
interface Props {
  entries: NetworkEntry[];
  onClear: () => void;
}
export function NetworkPanel({ entries, onClear }: Props) {
  return (
    <section className="flex min-h-0 flex-1 flex-col" aria-label="Rede">
      <div className="flex items-center justify-between border-b border-neutral-800 px-3 py-2 text-xs text-neutral-400">
        <span>{entries.length} requisições</span>
        <button className="rounded px-2 py-1 hover:bg-neutral-800" onClick={onClear}>
          Limpar rede
        </button>
      </div>
      <p className="px-3 py-2 text-xs text-neutral-500">
        Tráfego HTTP do runtime. Valores de consulta e credenciais ficam ocultos.
      </p>
      <div className="min-h-0 flex-1 overflow-auto">
        {entries.length === 0 ? (
          <p className="p-3 text-xs text-neutral-500">Sem requisições.</p>
        ) : (
          <table className="w-full table-fixed text-left text-xs">
            <thead className="sticky top-0 bg-neutral-900 text-neutral-400">
              <tr>
                <th className="w-16 p-2">Método</th>
                <th className="p-2">URL</th>
                <th className="w-20 p-2">Status</th>
                <th className="w-16 p-2">Tempo</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => (
                <tr key={entry.id} className="border-t border-neutral-800 align-top">
                  <td className="p-2 font-mono">{entry.method}</td>
                  <td className="break-all p-2" title={`${entry.responseBytes} bytes`}>
                    {entry.url}
                  </td>
                  <td
                    className={`p-2 font-mono ${entry.status !== null && (entry.status < 0 || entry.status >= 400) ? "text-red-400" : "text-emerald-300"}`}
                  >
                    {entry.status === null
                      ? "Pendente"
                      : entry.status === -28
                        ? "Cancelada"
                        : entry.status}
                  </td>
                  <td className="p-2 font-mono">
                    {entry.durationMs === null ? "—" : `${Math.round(entry.durationMs)} ms`}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}
