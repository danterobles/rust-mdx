//! Textos de UI (español). Único lugar con literales visibles para el usuario (NFR-8).

pub const APP_TITLE: &str = "Mdx";
pub const UNTITLED: &str = "Sin título";

pub const FILTER_MARKDOWN: &str = "Markdown y texto";
pub const FILTER_ALL: &str = "Todos los archivos";
pub const DIALOG_OPEN_TITLE: &str = "Abrir archivo";
pub const DIALOG_SAVE_TITLE: &str = "Guardar como";

pub const STATUS_NEW: &str = "Documento nuevo";
pub const STATUS_OPENED: &str = "Abierto";
pub const STATUS_SAVED: &str = "Guardado";

pub const DIRTY_MARK: &str = "● ";
pub const UNSAVED_TITLE: &str = "Cambios sin guardar";
pub const UNSAVED_BODY: &str = "«{name}» tiene cambios sin guardar. ¿Qué desea hacer?";
pub const UNSAVED_SAVE: &str = "Guardar";
pub const UNSAVED_DISCARD: &str = "Descartar";
pub const UNSAVED_CANCEL: &str = "Cancelar";

pub const STATUS_LINE_COL: &str = "Ln {line}, Col {col}";
pub const STATUS_WORDS: &str = "{n} palabras";
pub const STATUS_WORD: &str = "1 palabra";
pub const EOL_LF: &str = "LF";
pub const EOL_CRLF: &str = "CRLF";
pub const ENCODING_UTF8: &str = "UTF-8";
pub const ENCODING_UTF8_BOM: &str = "UTF-8 con BOM";

pub const STATUS_LINK_UNSUPPORTED: &str = "Enlace no soportado: {url}";

pub const DIALOG_FOLDER_TITLE: &str = "Abrir carpeta";
pub const STATUS_FOLDER_OPENED: &str = "Carpeta abierta";
pub const EXPLORER_EMPTY: &str = "Sin carpeta abierta.\nUse Ctrl/Cmd+Shift+O para abrir una.";
pub const EXPLORER_LOADING: &str = "Cargando…";
pub const EXPLORER_EMPTY_DIR: &str = "(vacía)";
pub const FILTER_HIDDEN: &str = "Mostrar archivos ocultos";
pub const FILTER_GITIGNORE: &str = "Respetar .gitignore";
pub const FILTER_ONLY_MARKDOWN: &str = "Solo archivos Markdown";

pub const CTX_NEW_FILE: &str = "Nuevo archivo";
pub const CTX_NEW_FOLDER: &str = "Nueva carpeta";
pub const CTX_RENAME: &str = "Renombrar";
pub const CTX_DELETE: &str = "Eliminar";
pub const CTX_REVEAL: &str = "Revelar en el sistema";
pub const CTX_CLOSE: &str = "Cerrar menú";
pub const PROMPT_NEW_FILE: &str = "Nombre del archivo (sin extensión se añade .md)";
pub const PROMPT_NEW_FOLDER: &str = "Nombre de la carpeta";
pub const PROMPT_RENAME: &str = "Nuevo nombre";
pub const PROMPT_ACCEPT: &str = "Aceptar";
pub const PROMPT_CANCEL: &str = "Cancelar";
pub const DELETE_TITLE: &str = "Eliminar";
pub const DELETE_BODY: &str = "«{name}» se moverá a la papelera del sistema.";
pub const DELETE_CONFIRM: &str = "Mover a la papelera";
pub const STATUS_CREATED: &str = "Creado";
pub const STATUS_RENAMED: &str = "Renombrado";
pub const STATUS_TRASHED: &str = "Enviado a la papelera";

pub const DELETED_MARK: &str = " (eliminado)";
pub const BANNER_CHANGED: &str = "El archivo cambió en disco.";
pub const BANNER_RELOAD: &str = "Recargar";
pub const BANNER_KEEP: &str = "Conservar mío";
pub const STATUS_DELETED_ON_DISK: &str =
    "El archivo se eliminó del disco; su contenido se conserva y puede volver a guardarlo.";
pub const STATUS_RELOADED: &str = "Archivo recargado desde el disco";
pub const STATUS_WATCH_FAILED: &str = "No se pudo vigilar la carpeta";

pub const TAB_CLOSE: &str = "×";

pub const SIDEBAR_FILES: &str = "Archivos";
pub const SIDEBAR_OUTLINE: &str = "Esquema";
pub const OUTLINE_EMPTY: &str = "Sin encabezados";
pub const OUTLINE_BLANK_HEADING: &str = "(sin texto)";

pub const SYNC_SCROLL: &str = "Sincronizar scroll con el editor";

pub const SIDEBAR_SEARCH: &str = "Buscar";
pub const FIND_PLACEHOLDER: &str = "Buscar…";
pub const REPLACE_PLACEHOLDER: &str = "Reemplazar por…";
pub const FIND_PREV: &str = "↑";
pub const FIND_NEXT: &str = "↓";
pub const FIND_CLOSE: &str = "×";
pub const FIND_CASE: &str = "Aa";
pub const FIND_WORD: &str = "Palabra";
pub const FIND_REGEX: &str = ".*";
pub const FIND_REPLACE_ONE: &str = "Reemplazar";
pub const FIND_REPLACE_ALL: &str = "Reemplazar todo";
pub const FIND_NO_RESULTS: &str = "Sin resultados";
pub const FIND_COUNT: &str = "{current} de {total}";
pub const FIND_REPLACED: &str = "{n} reemplazos";
pub const FIND_REPLACED_ONE: &str = "1 reemplazo";
pub const WS_PLACEHOLDER: &str = "Buscar en la carpeta…";
pub const WS_SEARCH: &str = "Buscar";
pub const WS_CANCEL: &str = "Cancelar";
pub const WS_RUNNING: &str = "Buscando…";
pub const WS_NO_FOLDER: &str = "Abra una carpeta (Ctrl/Cmd+Shift+O) para buscar en ella.";
pub const WS_NO_RESULTS: &str = "Sin resultados en {files} archivos";
pub const WS_SUMMARY: &str = "{matches} resultados en {files} archivos";
pub const WS_TRUNCATED: &str = " (hay más; afine la búsqueda)";
pub const WS_CANCELLED: &str = " (cancelada: resultados parciales)";
