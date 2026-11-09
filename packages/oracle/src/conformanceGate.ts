/** Known deviations are exact exceptions; new failures and missing probes must fail CI. */
export interface KnownFailure {
  status: string;
  detail: string[];
}
export function conformanceErrors(
  output: string,
  exitCode: number | null,
  probes: string[],
  known: Record<string, KnownFailure>,
): string[] {
  const errors: string[] = [];
  const lines = output.split(/\r?\n/);
  const seen = new Set<string>();
  let failures = 0;
  for (let index = 0; index < lines.length; index++) {
    const match = /^(PASS|FAIL|SKIP|CALIB|BENCH)\s+(\S+)(.*)$/.exec(lines[index]!);
    if (!match) continue;
    const status = match[1]!;
    const name = match[2]!;
    const suffix = match[3]!;
    if (seen.has(name)) errors.push(`Duplicate result: ${name}`);
    seen.add(name);
    if (!probes.includes(name)) errors.push(`Unexpected probe: ${name}`);
    if (status === "SKIP") errors.push(`Skipped probe: ${name}`);
    if (status === "FAIL") {
      failures++;
      const expected = known[name];
      const detail: string[] = [];
      while (lines[index + 1]?.startsWith("      ")) detail.push(lines[++index]!.trim());
      if (
        !expected ||
        suffix.trim() !== expected.status ||
        JSON.stringify(detail) !== JSON.stringify(expected.detail)
      )
        errors.push(`Unexpected failure: ${name}${suffix}`);
    } else if (known[name])
      errors.push(`Known failure changed: ${name}; review and update the exception`);
  }
  for (const name of probes) if (!seen.has(name)) errors.push(`Missing probe: ${name}`);
  for (const name of Object.keys(known))
    if (!probes.includes(name)) errors.push(`Stale exception: ${name}`);
  if (exitCode !== (failures ? 1 : 0)) errors.push(`Unexpected runner exit: ${exitCode}`);
  if (!/^\d+ passed, \d+ failed, \d+ skipped$/m.test(output))
    errors.push("Runner summary is missing");
  return errors;
}
