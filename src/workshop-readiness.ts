export type Readiness = {
  loaded: boolean; model: string; target: string; boardIds: string[];
  consumerRunning: boolean; scanError: string; busy: boolean;
  scanning: boolean; activeJob: boolean; phase: string;
  inspecting: boolean; inspectedTarget: string; wiringConfirmed: boolean;
};

export type Wiring = "standard" | "mistake";
export type Inspection = {
  target: string; state: string; message: string; firmware: string | null; boardId: string | null;
  identity: { model: string; hardwareProfile: string; generation: number } | null;
  wiring: Wiring | null; displayHealth: string | null; startupSaved: boolean; readyToVerify: boolean;
};

export type InspectionAttempt = { target: string; attempts: number; retryAt: number | null };

// A board can enumerate before its firmware accepts serial replies. Retry a
// transient read failure, but do not keep opening a healthy or unresponsive
// board indefinitely. A manual check or a newly attached target starts fresh.
export function nextInspectionAttempt(previous: InspectionAttempt | null, target: string, now: number, manual: boolean): InspectionAttempt | null {
  if (manual || !previous || previous.target !== target) return { target, attempts: 1, retryAt: null };
  if (previous.retryAt !== null && now >= previous.retryAt) return { target, attempts: previous.attempts + 1, retryAt: null };
  return null;
}

export function failedInspectionAttempt(attempt: InspectionAttempt, now: number): InspectionAttempt {
  const delay = [2000, 5000][attempt.attempts - 1];
  return { ...attempt, retryAt: delay === undefined ? null : now + delay };
}

export function inspectionNeedsWiringConfirmation(inspection: Inspection | null, selected: Wiring): boolean {
  if (!inspection || inspection.state === "bootloader") return false;
  return inspection.wiring === null || inspection.wiring !== selected;
}

export function rememberedModel(value: string | null): string {
  return ["nimbus", "boo", "meep", "claw"].includes(value ?? "") ? value! : "";
}

// Automatic background discovery is intentionally not a blocker. The native
// command checks the current USB devices again before it writes anything.
export function programmingBlocker(s: Readiness): string | null {
  if (!s.loaded) return "Opening Workshop…";
  if (s.busy) return "Keep the board connected while Workshop finishes.";
  if (s.activeJob || s.phase === "complete") return "Finish this board’s checks or choose Prepare another board.";
  if (s.consumerRunning) return "DroidFun App is still running. Close it with the button above so Workshop can check this board.";
  if (!s.model) return "Choose your bot above.";
  if (s.scanError) return "USB could not be checked. Click Check again to try again.";
  if (s.scanning || s.inspecting) return "Checking USB and installed firmware…";
  if (!s.boardIds.length) return "Connect one board with a USB data cable.";
  if (s.boardIds.length > 1) return "Unplug the other boards so only the one you want to prepare remains.";
  if (!s.boardIds.includes(s.target)) return "Choose the connected board above.";
  if (s.inspectedTarget !== s.target) return "Check this board’s installed firmware before continuing.";
  if (!s.wiringConfirmed) return "Check the physical screen wiring and confirm the selected connections above.";
  return null;
}
