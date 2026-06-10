interface Rift {
  completions: Completions,
}

interface Completions {
  registerCommandSignature: (signatures: CommandSignature | CommandSignature[]) => void,
}

declare namespace console {
  function log(message: string): void;
  function err(message: string): void;
}
