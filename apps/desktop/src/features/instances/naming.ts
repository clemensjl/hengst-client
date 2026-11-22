import type { LoaderInfoDto } from "@/ipc/bindings/LoaderInfoDto";

/**
 * The name the create dialog suggests: the game version, then the loader's display name. Loader
 * names come only from the backend (`LoaderInfoDto.displayName`).
 */
export function suggestName(game: string, loader: LoaderInfoDto | null): string {
  return loader ? `${game} ${loader.displayName}` : game;
}
