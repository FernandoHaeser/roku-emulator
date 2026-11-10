import { useEffect, useState } from "react";
type Registry = Record<string, Record<string, string>>;
interface Props {
  active: boolean;
  inspect: () => Promise<Registry>;
  update: (section: string, key: string, value: string | null) => Promise<Registry>;
}
const inputStyle = "min-w-0 rounded border border-neutral-700 bg-neutral-950 px-2 py-1 text-xs";
export function RegistryPanel({ active, inspect, update }: Props) {
  const [registry, setRegistry] = useState<Registry>({});
  const [error, setError] = useState("");
  const [section, setSection] = useState("");
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [reveal, setReveal] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!active || busy) return;
    let mounted = true;
    const refresh = async () => {
      try {
        const result = await inspect();
        if (mounted) {
          setRegistry(result);
          setError("");
        }
      } catch (error) {
        if (mounted) setError(String(error));
      }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 1500);
    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, [active, inspect, busy]);
  const change = async (section: string, key: string, value: string | null) => {
    setBusy(true);
    try {
      setRegistry(await update(section, key, value));
      setError("");
    } catch (error) {
      setError(String(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section aria-label="Registro" className="flex min-h-0 flex-1 flex-col text-xs">
      <label className="flex gap-2 border-b border-neutral-800 p-3 text-neutral-400">
        <input type="checkbox" checked={reveal} onChange={(e) => setReveal(e.target.checked)} />
        Mostrar valores
      </label>
      {error && (
        <p role="alert" className="p-3 text-red-400">
          {error}
        </p>
      )}
      <div className="min-h-0 flex-1 overflow-auto p-3">
        {!active ? (
          <p className="text-neutral-500">Inicie um canal para inspecionar seu registro.</p>
        ) : Object.entries(registry).length === 0 ? (
          <p className="text-neutral-500">Registro vazio.</p>
        ) : (
          Object.entries(registry).map(([section, entries]) => (
            <fieldset key={section} className="mb-4 min-w-0 border border-neutral-800 p-2">
              <legend className="break-all px-1 font-semibold">{section}</legend>
              {Object.entries(entries).map(([key, value]) => (
                <div key={key} className="my-2 grid min-w-0 grid-cols-[1fr_auto_auto] gap-2">
                  <div className="min-w-0">
                    <div className="break-all text-neutral-400">{key}</div>
                    <input
                      className={`${inputStyle} w-full`}
                      type={reveal ? "text" : "password"}
                      readOnly
                      aria-label={`Valor de ${section}/${key}`}
                      value={value}
                    />
                  </div>
                  <button
                    disabled={busy}
                    onClick={() => {
                      setSection(section);
                      setKey(key);
                      setValue(value);
                    }}
                  >
                    Editar
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => void change(section, key, null)}
                    aria-label={`Excluir ${section}/${key}`}
                    className="text-red-400"
                  >
                    Excluir
                  </button>
                </div>
              ))}
            </fieldset>
          ))
        )}
      </div>
      <form
        className="grid gap-2 border-t border-neutral-800 p-3"
        onSubmit={(event) => {
          event.preventDefault();
          void change(section, key, value);
        }}
      >
        <label className="grid gap-1">
          Seção
          <input
            className={inputStyle}
            required
            value={section}
            onChange={(e) => setSection(e.target.value)}
          />
        </label>
        <label className="grid gap-1">
          Chave
          <input
            className={inputStyle}
            required
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
        <label className="grid gap-1">
          Valor
          <input
            className={inputStyle}
            type={reveal ? "text" : "password"}
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
        </label>
        <button
          disabled={!active || busy}
          className="rounded bg-violet-700 px-3 py-2 disabled:opacity-40"
        >
          Salvar no registro
        </button>
      </form>
    </section>
  );
}
