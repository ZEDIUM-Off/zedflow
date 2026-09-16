import { zipSync } from 'fflate'
import { generatedFileBytes, type GeneratedFile } from '@zedflow/sdk'

export type ExportFile = GeneratedFile

/** Preserve source and binary package bytes in the downloaded Cargo project. */
export function cargoProjectArchive(files: ExportFile[]): Uint8Array<ArrayBuffer> {
  return zipSync(Object.fromEntries(files.map(file => [file.path, generatedFileBytes(file)])), { level: 6 })
}

export function downloadCargoProject(files: ExportFile[], filename: string) {
  const archive = cargoProjectArchive(files)
  const url = URL.createObjectURL(new Blob([archive], { type: 'application/zip' }))
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.append(link)
  link.click()
  link.remove()
  // Let the browser begin reading the blob before releasing its URL.
  window.setTimeout(() => URL.revokeObjectURL(url), 1000)
}
