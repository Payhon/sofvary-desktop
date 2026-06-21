import { open, save } from "@tauri-apps/plugin-dialog";

interface DialogFilter {
  name: string;
  extensions: string[];
}

interface SaveDialogOptions {
  title: string;
  defaultPath: string;
  filters: DialogFilter[];
}

interface OpenFileDialogOptions {
  title: string;
  filters: DialogFilter[];
}

interface OpenDirectoryDialogOptions {
  title: string;
}

export async function saveFilePath(options: SaveDialogOptions): Promise<string | null> {
  return save(options);
}

export async function openSingleFilePath(options: OpenFileDialogOptions): Promise<string | null> {
  const selectedPath = await open({
    title: options.title,
    multiple: false,
    directory: false,
    filters: options.filters,
  });

  return Array.isArray(selectedPath) ? selectedPath[0] ?? null : selectedPath;
}

export async function openDirectoryPath(options: OpenDirectoryDialogOptions): Promise<string | null> {
  const selectedPath = await open({
    title: options.title,
    multiple: false,
    directory: true,
  });

  return Array.isArray(selectedPath) ? selectedPath[0] ?? null : selectedPath;
}
