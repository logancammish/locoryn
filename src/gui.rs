use iced::{
    Background, Border, Color, Element, Length, Shadow, Theme, Vector,
    alignment::Horizontal,
    border::Radius,
    mouse,
    widget::{self, Space, container},
};

use iced_selection::markdown as selectable_markdown;
use iced_widget::{container::Style, core::text::Wrapping, markdown};
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    AppUpdateState, ChatImage, Correspondence, GUIState, Language, MarkdownImageState, Message,
    Program, SettingsFeedbackTarget, ThinkingLevel, split_thinking_text,
    web_search::{WebSearchState, WebSource},
};

/// Spanish is intentionally shipped as an experimental machine-generated
/// translation. Unknown/new strings fall back to English instead of disappearing.
fn tr(language: Language, english: &'static str) -> &'static str {
    if language == Language::English {
        return english;
    }
    match english {
        "A polished desktop interface for chatting with local Ollama models." => {
            "Una interfaz de escritorio cuidada para conversar con modelos locales de Ollama."
        }
        "HELP" => "AYUDA",
        "Chat locally" => "Chat local",
        "Select one of your installed Ollama models, type a prompt, and press Enter to generate a response." => {
            "Selecciona uno de tus modelos de Ollama instalados, escribe un mensaje y pulsa Intro para generar una respuesta."
        }
        "Manage models" => "Gestionar modelos",
        "Use Advanced Settings to install models by name, change the Ollama address, or tune response rendering." => {
            "Usa la configuración avanzada para instalar modelos por nombre, cambiar la dirección de Ollama o ajustar la presentación de respuestas."
        }
        "System prompts" => "Indicaciones del sistema",
        "System prompts let you switch the assistant's behaviour or personality without rewriting your prompt each time." => {
            "Las indicaciones del sistema permiten cambiar el comportamiento o la personalidad del asistente sin reescribirlas cada vez."
        }
        "Chat history" => "Historial de chats",
        "When enabled, conversations can be saved locally. You can wipe the current history from Settings." => {
            "Cuando está activado, las conversaciones se guardan localmente. Puedes borrar el contexto actual en Configuración."
        }
        "Files and configuration" => "Archivos y configuración",
        "User settings, logs, generated images, and chats are stored in your local application-data folder. Installed assets remain read-only." => {
            "Los ajustes, registros, imágenes generadas y chats se guardan en la carpeta local de datos de la aplicación. Los recursos instalados son de solo lectura."
        }
        "Back to chat" => "Volver al chat",
        "No model selected" => "Ningún modelo seleccionado",
        "Ask something..." => "Escribe algo...",
        "New conversation" => "Nueva conversación",
        "What can I help you make?" => "¿Qué te gustaría crear?",
        "Choose a starting point below, or write your own message." => {
            "Elige un punto de partida o escribe tu propio mensaje."
        }
        "No models installed" => "No hay modelos instalados",
        "Thinking" => "Razonamiento",
        "Ready when you are." => "Listo cuando quieras.",
        "Choose a model, type a prompt, and start chatting locally." => {
            "Elige un modelo, escribe un mensaje y empieza a conversar localmente."
        }
        "Ollama was not detected." => "No se detectó Ollama.",
        "Install Ollama or check your connection settings." => {
            "Instala Ollama o revisa la configuración de conexión."
        }
        "Install Ollama" => "Instalar Ollama",
        "No models were detected." => "No se detectaron modelos.",
        "Install a model before sending prompts." => "Instala un modelo antes de enviar mensajes.",
        "Find models" => "Buscar modelos",
        "＋ New chat" => "＋ Nuevo chat",
        "Leave temporary chat" => "Salir del chat temporal",
        "Temporary chat" => "Chat temporal",
        "Temporary chats" => "Chats temporales",
        "Temporary · not saved" => "Temporal · no guardado",
        "Saved chats" => "Chats guardados",
        "Unpin" => "Desfijar",
        "Pin" => "Fijar",
        "Chats" => "Chats",
        "Local workspace" => "Espacio local",
        "LOCAL AI WORKSPACE" => "ESPACIO DE IA LOCAL",
        "Online" => "En línea",
        "Offline" => "Sin conexión",
        "Images" => "Imágenes",
        "▧ Images" => "▧ Imágenes",
        "＋ Image" => "＋ Imagen",
        "＋ Attach" => "＋ Adjuntar",
        "Paste" => "Pegar",
        "Settings" => "Configuración",
        "⚙ Settings" => "⚙ Configuración",
        "Enable Web Search" => "Activar búsqueda web",
        "Web search may send search queries and webpage URLs to the selected external provider." => {
            "La búsqueda web puede enviar consultas y direcciones de páginas al proveedor externo seleccionado."
        }
        "Search provider" => "Proveedor de búsqueda",
        "API key" => "Clave de API",
        "Prefer BRAVE_SEARCH_API_KEY for secret storage. A key entered here is stored in the local settings file and never printed in logs." => {
            "Es preferible usar BRAVE_SEARCH_API_KEY. Las claves introducidas aquí se guardan en el archivo local de configuración y nunca se muestran en los registros."
        }
        "Search result limit" => "Límite de resultados",
        "Deep follow-up research" => "Investigación de seguimiento exhaustiva",
        "Deep research controls" => "Controles de investigación exhaustiva",
        "After web research starts, the model runs 3–6 targeted searches and checks 2–6 relevant pages across independent sites." => {
            "Cuando comienza la investigación web, el modelo realiza de 3 a 6 búsquedas específicas y comprueba de 2 a 6 páginas relevantes de sitios independientes."
        }
        "Web search" => "Búsqueda web",
        "Web on" => "Web activada",
        "Web off" => "Web desactivada",
        "Searching the web…" => "Buscando en la web…",
        "Fetching webpage…" => "Leyendo página web…",
        "Web search activity" => "Actividad de búsqueda web",
        "Searching" => "Buscando",
        "Reviewing results" => "Revisando resultados",
        "Reading website" => "Leyendo sitio web",
        "found" => "encontrados",
        "Websites found" => "Sitios encontrados",
        "The model is choosing which result to read." => {
            "El modelo está eligiendo qué resultado leer."
        }
        "Preparing the answer from these sources." => "Preparando la respuesta con estas fuentes.",
        "Search query" => "Consulta",
        "Details" => "Detalles",
        "ERROR" => "ERROR",
        "READING" => "LEYENDO",
        "Sources" => "Fuentes",
        "WEB" => "WEB",
        "MODEL" => "MODELO",
        "SYSTEM PROMPT" => "INDICACIÓN DEL SISTEMA",
        "System prompt" => "Indicación del sistema",
        "Dynamic system prompt" => "Indicación dinámica del sistema",
        "Add current local information and your own instructions to every request." => {
            "Añade información local actual y tus propias instrucciones a cada solicitud."
        }
        "Include date" => "Incluir fecha",
        "Include time" => "Incluir hora",
        "Include user name" => "Incluir nombre del usuario",
        "User name" => "Nombre del usuario",
        "Custom instructions appended to the system prompt" => {
            "Instrucciones personalizadas añadidas a la indicación del sistema"
        }
        "Local code checking" => "Comprobación local de código",
        "Allow generated Python, Rust, C, C++, and C# snippets to be checked with locally installed tools." => {
            "Permite comprobar fragmentos generados de Python, Rust, C, C++ y C# con herramientas instaladas localmente."
        }
        "Warning: checking invokes local compilers or interpreters on generated code. It can fail, consume resources, or cause unintended errors. Enable it only when you consent and trust the code." => {
            "Advertencia: la comprobación ejecuta compiladores o intérpretes locales sobre código generado. Puede fallar, consumir recursos o causar errores imprevistos. Actívala solo si das tu consentimiento y confías en el código."
        }
        "REASONING" => "RAZONAMIENTO",
        "Stop" => "Detener",
        "■ Stop" => "■ Detener",
        "Send" => "Enviar",
        "Enter to send" => "Intro para enviar",
        "Enter to send · Shift+Enter for a new line" => {
            "Intro para enviar · Mayús+Intro para una línea nueva"
        }
        "Paste image" => "Pegar imagen",
        "Copy response" => "Copiar respuesta",
        "Remove" => "Quitar",
        "Copied ✓" => "Copiado ✓",
        "Copy code" => "Copiar código",
        "Check code" => "Comprobar código",
        "You" => "Tú",
        "▾ Hide thinking" => "▾ Ocultar razonamiento",
        "▸ Show thinking" => "▸ Mostrar razonamiento",
        "Describe an image, or ask a question about the attached image…" => {
            "Describe una imagen o pregunta sobre la imagen adjunta…"
        }
        "Add an image for vision" => "Añade una imagen para visión",
        "Paste from the clipboard or choose a local image." => {
            "Pega desde el portapapeles o elige una imagen local."
        }
        "Choose image" => "Elegir imagen",
        "Copy image" => "Copiar imagen",
        "Vision model is responding…" => "El modelo de visión está respondiendo…",
        "Vision response" => "Respuesta de visión",
        "Generate image" => "Generar imagen",
        "Generating…" => "Generando…",
        "Describe the image you want to generate…" => "Describe la imagen que quieres generar…",
        "Use vision models to inspect images or experimental image models to create them." => {
            "Usa modelos de visión para analizar imágenes o modelos experimentales para crearlas."
        }
        "Model" => "Modelo",
        "Ask about image" => "Preguntar sobre la imagen",
        "Generation requires an Ollama image-generation model and supported runtime." => {
            "La generación requiere un modelo de imágenes de Ollama y un entorno compatible."
        }
        "Analyze images with a vision model. Experimental image generation appears only for models that report support." => {
            "Analiza imágenes con un modelo de visión. La generación experimental solo aparece en modelos compatibles."
        }
        "Vision analysis" => "Análisis visual",
        "Attach an image and ask a vision-capable model to describe, classify, read, or reason about it." => {
            "Adjunta una imagen y pide a un modelo con visión que la describa, clasifique, lea o analice."
        }
        "This model can inspect images." => "Este modelo puede analizar imágenes.",
        "This model does not support image input." => "Este modelo no admite imágenes de entrada.",
        "Checking image capabilities…" => "Comprobando capacidades de imagen…",
        "Experimental image generation" => "Generación de imágenes experimental",
        "Ollama reports that this model can generate images. Output is requested through /v1/images/generations at 1024 × 1024." => {
            "Ollama indica que este modelo puede generar imágenes. Se solicita la salida mediante /v1/images/generations a 1024 × 1024."
        }
        "Generated images" => "Imágenes generadas",
        "Tune model behaviour, prompt selection, and chat preferences." => {
            "Ajusta el comportamiento del modelo, las indicaciones y las preferencias del chat."
        }
        "PERSONALIZATION" => "PERSONALIZACIÓN",
        "MODEL & RESPONSES" => "MODELO Y RESPUESTAS",
        "APPEARANCE" => "APARIENCIA",
        "WEB SEARCH & TOOLS" => "BÚSQUEDA WEB Y HERRAMIENTAS",
        "DATA & MAINTENANCE" => "DATOS Y MANTENIMIENTO",
        "MODELS & SAFETY" => "MODELOS Y SEGURIDAD",
        "RUNTIME & CONNECTION" => "ENTORNO Y CONEXIÓN",
        "Application updates" => "Actualizaciones de la aplicación",
        "Current version and latest stable release from GitHub." => {
            "Versión actual y última versión estable de GitHub."
        }
        "Check now" => "Buscar ahora",
        "Checking…" => "Buscando…",
        "Download update" => "Descargar actualización",
        "Go back" => "Volver",
        "Choose the Ollama model used for new responses." => {
            "Elige el modelo de Ollama para las respuestas nuevas."
        }
        "Thinking effort" => "Nivel de razonamiento",
        "Choose how much reasoning the model should use." => {
            "Elige cuánto razonamiento debe usar el modelo."
        }
        "Reasoning" => "Razonamiento",
        "This model does not offer adjustable reasoning." => {
            "Este modelo no ofrece razonamiento ajustable."
        }
        "Select a model and wait while reasoning support is checked." => {
            "Selecciona un modelo mientras se comprueba la compatibilidad con razonamiento."
        }
        "Temperature" => "Temperatura",
        "Higher values make output more random." => {
            "Los valores altos producen respuestas más aleatorias."
        }
        "Maximum response" => "Respuesta máxima",
        "Caps generated output in tokens, including hidden reasoning. The default is 32,768; direct entry supports up to 1,048,576." => {
            "Limita la salida generada en tokens, incluido el razonamiento oculto. El valor predeterminado es 32.768; la entrada directa admite hasta 1.048.576."
        }
        "Context window" => "Ventana de contexto",
        "Controls how much conversation and generated output the model can hold. Larger values use substantially more memory." => {
            "Controla cuánta conversación y salida puede mantener el modelo. Los valores grandes usan bastante más memoria."
        }
        "Choose the personality or instruction profile." => {
            "Elige el perfil de personalidad o instrucciones."
        }
        "Text size" => "Tamaño del texto",
        "Adjust chat and response readability." => {
            "Ajusta la legibilidad del chat y las respuestas."
        }
        "Dark mode" => "Modo oscuro",
        "Switch between the dark and light interface themes." => {
            "Cambia entre los temas oscuro y claro de la interfaz."
        }
        "Chat storage" => "Almacenamiento de chats",
        "Saved chats use this folder. The full path is shown so you can always locate them." => {
            "Los chats guardados usan esta carpeta. Se muestra la ruta completa para que puedas encontrarlos."
        }
        "Choose folder" => "Elegir carpeta",
        "Model conversation context" => "Contexto de conversación del modelo",
        "Include earlier messages from this chat in the next model request. Saved chats are managed in the left menu." => {
            "Incluye mensajes anteriores de este chat en la próxima solicitud. Los chats guardados se gestionan en el menú izquierdo."
        }
        "Enabled" => "Activado",
        "Interface language" => "Idioma de la interfaz",
        "Spanish is experimental and machine-generated. It will be replaced with a human translation in a future update." => {
            "El español es experimental y ha sido generado automáticamente. Se sustituirá por una traducción humana en una actualización futura."
        }
        "Maintenance" => "Mantenimiento",
        "Clear local conversation data or open deeper configuration options." => {
            "Borra el contexto local o abre opciones de configuración adicionales."
        }
        "Clear current context" => "Borrar contexto actual",
        "Advanced settings" => "Configuración avanzada",
        "Model name, e.g. llama3.2:3b" => "Nombre del modelo, p. ej. llama3.2:3b",
        "Install models, change connection settings, and tune rendering." => {
            "Instala modelos, cambia la conexión y ajusta la presentación."
        }
        "Back to settings" => "Volver a configuración",
        "Change the active prompt profile." => "Cambia el perfil de indicaciones activo.",
        "Install model" => "Instalar modelo",
        "Enter an Ollama model name and press Enter." => {
            "Escribe el nombre de un modelo de Ollama y pulsa Intro."
        }
        "Batch tokens" => "Lote de tokens",
        "Tokens per visual update when fast streaming is off. Higher values reduce rendering work." => {
            "Tokens por actualización visual cuando la transmisión rápida está desactivada. Los valores altos reducen el trabajo de presentación."
        }
        "Fast streaming" => "Transmisión rápida",
        "Render as soon as the API yields output. Turn off to use token batching." => {
            "Muestra la respuesta en cuanto la API produce contenido. Desactívalo para usar lotes de tokens."
        }
        "Show tokens per second at bottom of message" => {
            "Mostrar tokens por segundo al final del mensaje"
        }
        "Display the generation speed under each assistant reply. Measured from the model's own statistics, independent of token batching." => {
            "Muestra la velocidad de generación bajo cada respuesta. Se mide con las estadísticas del propio modelo, sin verse afectada por el lote de tokens."
        }
        "Content filtering" => "Filtro de contenido",
        "Censor offensive, profane, sexual, and severely inappropriate words with # characters." => {
            "Censura palabras ofensivas, malsonantes, sexuales y gravemente inapropiadas con caracteres #."
        }
        "Ollama address" => "Dirección de Ollama",
        "Change the IP address and port used to connect to Ollama." => {
            "Cambia la dirección IP y el puerto usados para conectar con Ollama."
        }
        "Fastest · no extra reasoning" => "Más rápido · sin razonamiento adicional",
        "Use this model's standard reasoning mode" => {
            "Usa el modo de razonamiento estándar de este modelo"
        }
        "Minimal reasoning for very quick responses" => {
            "Razonamiento mínimo para respuestas muy rápidas"
        }
        "Quick reasoning for everyday questions" => "Razonamiento rápido para preguntas cotidianas",
        "Balanced for multi-step tasks" => "Equilibrado para tareas de varios pasos",
        "Most thorough · slower responses" => "Más exhaustivo · respuestas más lentas",
        "Extra-deep reasoning for difficult tasks" => {
            "Razonamiento extra profundo para tareas difíciles"
        }
        "Maximum reasoning the model offers" => "Máximo razonamiento que ofrece el modelo",
        _ => english,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ThinkingChoice {
    level: ThinkingLevel,
    language: Language,
}

impl ThinkingChoice {
    fn from_levels(levels: &[ThinkingLevel], language: Language) -> Vec<Self> {
        levels
            .iter()
            .copied()
            .map(|level| Self { level, language })
            .collect()
    }
}

impl fmt::Display for ThinkingChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match (self.language, self.level) {
            (Language::Spanish, ThinkingLevel::Off) => "Desactivado",
            (Language::Spanish, ThinkingLevel::On) => "Activado",
            (Language::Spanish, ThinkingLevel::Minimal) => "Mínimo",
            (Language::Spanish, ThinkingLevel::Low) => "Bajo",
            (Language::Spanish, ThinkingLevel::Medium) => "Medio",
            (Language::Spanish, ThinkingLevel::High) => "Alto",
            (Language::Spanish, ThinkingLevel::XHigh) => "Extra alto",
            (Language::Spanish, ThinkingLevel::Max) => "Máximo",
            (_, ThinkingLevel::Off) => "Off",
            (_, ThinkingLevel::On) => "On",
            (_, ThinkingLevel::Minimal) => "Minimal",
            (_, ThinkingLevel::Low) => "Low",
            (_, ThinkingLevel::Medium) => "Medium",
            (_, ThinkingLevel::High) => "High",
            (_, ThinkingLevel::XHigh) => "Extra high",
            (_, ThinkingLevel::Max) => "Maximum",
        };
        formatter.write_str(label)
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

static DARK_MODE: AtomicBool = AtomicBool::new(true);

pub(crate) fn set_dark_mode(enabled: bool) {
    DARK_MODE.store(enabled, Ordering::Relaxed);
}

fn is_dark_mode() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

fn app_bg() -> Color {
    if is_dark_mode() {
        rgb(8, 10, 15)
    } else {
        rgb(246, 247, 251)
    }
}

fn panel() -> Color {
    if is_dark_mode() {
        rgb(15, 18, 26)
    } else {
        rgb(255, 255, 255)
    }
}

fn panel_soft() -> Color {
    if is_dark_mode() {
        rgb(20, 24, 34)
    } else {
        rgb(248, 249, 252)
    }
}

fn panel_lifted() -> Color {
    if is_dark_mode() {
        rgb(25, 30, 42)
    } else {
        rgb(252, 252, 254)
    }
}

fn border_soft() -> Color {
    if is_dark_mode() {
        rgb(42, 48, 64)
    } else {
        rgb(222, 225, 234)
    }
}

fn border_bright() -> Color {
    if is_dark_mode() {
        rgb(79, 88, 116)
    } else {
        rgb(164, 170, 190)
    }
}

fn text_main() -> Color {
    if is_dark_mode() {
        rgb(241, 242, 247)
    } else {
        rgb(29, 31, 40)
    }
}

fn text_muted() -> Color {
    if is_dark_mode() {
        rgb(161, 167, 184)
    } else {
        rgb(91, 95, 111)
    }
}

fn text_faint() -> Color {
    if is_dark_mode() {
        rgb(111, 117, 134)
    } else {
        rgb(119, 123, 139)
    }
}

fn accent() -> Color {
    rgb(139, 124, 246)
}

fn accent_2() -> Color {
    rgb(87, 214, 198)
}

fn danger() -> Color {
    rgb(255, 92, 116)
}

fn success() -> Color {
    rgb(91, 211, 157)
}

fn warning() -> Color {
    rgb(255, 190, 94)
}

fn shadow_color() -> Color {
    Color {
        a: if is_dark_mode() { 0.30 } else { 0.12 },
        ..rgb(0, 0, 0)
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}

fn eased(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn app_background_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(app_bg())),
        border: Border {
            color: app_bg(),
            width: 0.0,
            radius: Radius::from(0.0),
        },
        shadow: Shadow::default(),
    }
}

fn sidebar_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(12, 15, 22)
        } else {
            rgb(251, 251, 253)
        })),
        border: Border {
            color: border_soft(),
            width: 1.0,
            radius: Radius::from(20.0),
        },
        shadow: Shadow {
            color: shadow_color(),
            offset: Vector::from([0.0, 8.0]),
            blur_radius: 26.0,
        },
    }
}

fn top_bar_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel())),
        border: Border {
            color: border_soft(),
            width: 1.0,
            radius: Radius::from(18.0),
        },
        shadow: Shadow {
            color: Color {
                a: if is_dark_mode() { 0.18 } else { 0.06 },
                ..rgb(0, 0, 0)
            },
            offset: Vector::from([0.0, 4.0]),
            blur_radius: 16.0,
        },
    }
}

fn conversation_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(11, 14, 20)
        } else {
            rgb(252, 252, 254)
        })),
        border: Border {
            color: if is_dark_mode() {
                rgb(27, 32, 44)
            } else {
                rgb(232, 234, 241)
            },
            width: 1.0,
            radius: Radius::from(20.0),
        },
        shadow: Shadow::default(),
    }
}

fn composer_style(active: bool, pulse: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel())),
        border: Border {
            color: if active {
                mix_color(border_bright(), accent(), 0.42 + pulse * 0.07)
            } else if is_dark_mode() {
                rgb(62, 66, 88)
            } else {
                rgb(211, 213, 224)
            },
            width: if active { 1.4 } else { 1.0 },
            radius: Radius::from(18.0),
        },
        shadow: Shadow {
            color: if active {
                with_alpha(accent(), 0.09 + pulse * 0.025)
            } else {
                shadow_color()
            },
            offset: Vector::from([0.0, 8.0 + pulse]),
            blur_radius: 22.0 + pulse * 3.0,
        },
    }
}

fn pick_list_style(_theme: &Theme, status: widget::pick_list::Status) -> widget::pick_list::Style {
    let active = !matches!(status, widget::pick_list::Status::Active);
    widget::pick_list::Style {
        text_color: text_main(),
        placeholder_color: text_faint(),
        handle_color: if active { accent() } else { text_muted() },
        background: Background::Color(if active {
            if is_dark_mode() {
                rgb(29, 37, 57)
            } else {
                rgb(232, 237, 248)
            }
        } else {
            panel_soft()
        }),
        border: Border {
            color: if active { accent() } else { border_soft() },
            width: if active { 1.5 } else { 1.0 },
            radius: Radius::from(12.0),
        },
    }
}

fn pick_list_menu_style(_theme: &Theme) -> widget::overlay::menu::Style {
    widget::overlay::menu::Style {
        background: Background::Color(panel_lifted()),
        border: Border {
            color: border_bright(),
            width: 1.0,
            radius: Radius::from(14.0),
        },
        text_color: text_main(),
        selected_text_color: Color::WHITE,
        selected_background: Background::Color(if is_dark_mode() {
            rgb(49, 67, 122)
        } else {
            rgb(75, 99, 205)
        }),
        shadow: Shadow {
            color: shadow_color(),
            offset: Vector::from([0.0, 8.0]),
            blur_radius: 22.0,
        },
    }
}

fn flat_card_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel_lifted())),
        border: Border {
            color: border_soft(),
            width: 1.0,
            radius: Radius::from(14.0),
        },
        shadow: Shadow::default(),
    }
}

fn chat_entry_style(active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if active {
            if is_dark_mode() {
                rgb(43, 38, 76)
            } else {
                rgb(239, 236, 253)
            }
        } else {
            Color::TRANSPARENT
        })),
        border: Border {
            color: if active {
                Color {
                    a: 0.72,
                    ..accent()
                }
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow::default(),
    }
}

fn chat_title_button_style(
    _theme: &Theme,
    status: widget::button::Status,
) -> widget::button::Style {
    widget::button::Style {
        snap: true,
        background: match status {
            widget::button::Status::Hovered => {
                Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.05)))
            }
            _ => None,
        },
        text_color: text_main(),
        border: Border {
            radius: Radius::from(10.0),
            ..Border::default()
        },
        shadow: Shadow::default(),
    }
}

fn input_shell_style(active: bool, pulse: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(18, 22, 31)
        } else {
            rgb(248, 249, 252)
        })),
        border: Border {
            color: if active {
                mix_color(border_bright(), accent(), 0.25 + pulse * 0.05)
            } else if is_dark_mode() {
                rgb(49, 55, 73)
            } else {
                rgb(222, 224, 232)
            },
            width: if active { 1.25 } else { 1.0 },
            radius: Radius::from(14.0),
        },
        shadow: Shadow::default(),
    }
}

fn user_bubble_style(reveal: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| {
        let target = if is_dark_mode() {
            rgb(56, 48, 101)
        } else {
            rgb(235, 231, 252)
        };
        Style {
            snap: true,
            text_color: Some(if is_dark_mode() {
                Color::WHITE
            } else {
                text_main()
            }),
            background: Some(Background::Color(mix_color(app_bg(), target, reveal))),
            border: Border {
                color: with_alpha(accent(), 0.55 * reveal),
                width: 1.0,
                radius: Radius::from(16.0),
            },
            shadow: Shadow {
                color: with_alpha(shadow_color(), shadow_color().a * reveal),
                offset: Vector::from([0.0, 2.0 + reveal * 2.0]),
                blur_radius: 4.0 + reveal * 8.0,
            },
        }
    }
}

fn bot_bubble_style(reveal: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| {
        let target = if is_dark_mode() {
            rgb(17, 21, 29)
        } else {
            rgb(255, 255, 255)
        };
        Style {
            snap: true,
            text_color: Some(text_main()),
            background: Some(Background::Color(mix_color(app_bg(), target, reveal))),
            border: Border {
                color: with_alpha(
                    if is_dark_mode() {
                        rgb(35, 41, 54)
                    } else {
                        rgb(231, 233, 239)
                    },
                    reveal,
                ),
                width: 1.0,
                radius: Radius::from(16.0),
            },
            shadow: Shadow {
                color: with_alpha(
                    rgb(0, 0, 0),
                    if is_dark_mode() {
                        0.14 * reveal
                    } else {
                        0.05 * reveal
                    },
                ),
                offset: Vector::from([0.0, 3.0]),
                blur_radius: 4.0 + reveal * 8.0,
            },
        }
    }
}

fn web_activity_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(17, 27, 39)
        } else {
            rgb(235, 248, 252)
        })),
        border: Border {
            color: rgb(48, 112, 139),
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow::default(),
    }
}

fn website_row_style(active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if active {
            if is_dark_mode() {
                rgb(27, 55, 68)
            } else {
                rgb(219, 242, 247)
            }
        } else {
            panel_soft()
        })),
        border: Border {
            color: if active { accent_2() } else { border_soft() },
            width: 1.0,
            radius: Radius::from(10.0),
        },
        shadow: Shadow::default(),
    }
}

fn chip_style(color: Color) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel_soft())),
        border: Border {
            color,
            width: 1.0,
            radius: Radius::from(999.0),
        },
        shadow: Shadow::default(),
    }
}

fn feedback_chip_style(color: Color, bounce: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(mix_color(
            panel_soft(),
            color,
            bounce * 0.08,
        ))),
        border: Border {
            color: brighten(color, bounce * 0.08),
            width: 1.0 + bounce * 0.65,
            radius: Radius::from(999.0),
        },
        shadow: Shadow {
            color: with_alpha(color, bounce * 0.24),
            offset: Vector::from([0.0, bounce * 1.8]),
            blur_radius: bounce * 8.0,
        },
    }
}

fn feedback_value_chip(value: String, color: Color, bounce: f32) -> Element<'static, Message> {
    container(widget::text(value).size(13).color(text_main()))
        .padding(8.0 + bounce * 1.1)
        .style(feedback_chip_style(color, bounce))
        .into()
}

fn feedback_apply_button<'a>(
    label: &'a str,
    message: Message,
    bounce: f32,
) -> Element<'a, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([8.0 + bounce * 0.22, 10.0 + bounce * 0.35])
        .style(move |_theme, status| {
            let mut style = button_visual(panel_soft(), border_soft(), text_muted(), status);
            style.border.color = mix_color(style.border.color, accent_2(), bounce * 0.24);
            style.border.width += bounce * 0.16;
            style.shadow.color = with_alpha(accent_2(), bounce * 0.10);
            style.shadow.offset = Vector::from([0.0, 1.0 + bounce * 0.55]);
            style.shadow.blur_radius += bounce * 2.0;
            style
        })
        .on_press(message)
        .into()
}

/// The brand mark is intentionally static and squircle-shaped: a square with
/// heavily rounded corners, never a circle.
fn status_brand_style(color: Color) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: None,
        background: Some(Background::Color(with_alpha(color, 0.035))),
        border: Border {
            color: brighten(color, 0.04),
            width: 1.2,
            radius: Radius::from(10.0),
        },
        shadow: Shadow {
            color: with_alpha(color, 0.13),
            offset: Vector::from([0.0, 1.0]),
            blur_radius: 5.0,
        },
    }
}

fn profile_chip_style(
    open: bool,
) -> impl Fn(&Theme, widget::button::Status) -> widget::button::Style {
    move |_theme, status| {
        let hovered = matches!(status, widget::button::Status::Hovered);
        widget::button::Style {
            snap: true,
            background: Some(Background::Color(if open || hovered {
                panel_lifted()
            } else {
                panel()
            })),
            text_color: text_main(),
            border: Border {
                color: border_soft(),
                width: 1.0,
                radius: Radius::from(10.0),
            },
            shadow: Shadow {
                color: with_alpha(shadow_color(), 0.28),
                offset: Vector::from([0.0, 2.0]),
                blur_radius: 6.0,
            },
        }
    }
}

fn profile_popup_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: None,
        background: Some(Background::Color(panel_lifted())),
        border: Border {
            color: border_soft(),
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow {
            color: with_alpha(shadow_color(), 0.38),
            offset: Vector::from([0.0, 6.0]),
            blur_radius: 16.0,
        },
    }
}

fn resize_rail_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: None,
        background: Some(Background::Color(with_alpha(border_bright(), 0.62))),
        border: Border {
            color: with_alpha(accent(), 0.18),
            width: 1.0,
            radius: Radius::from(999.0),
        },
        shadow: Shadow::default(),
    }
}

fn sidebar_resize_handle<'a>() -> Element<'a, Message> {
    widget::mouse_area(
        container(
            container(Space::new())
                .width(Length::Fixed(2.0))
                .height(Length::Fill)
                .style(resize_rail_style),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fixed(10.0))
        .height(Length::Fill),
    )
    .on_press(Message::StartUiResize(crate::UiResizeTarget::Sidebar))
    .on_release(Message::StopUiResize)
    .interaction(mouse::Interaction::ResizingHorizontally)
    .into()
}

fn composer_resize_handle<'a>() -> Element<'a, Message> {
    widget::mouse_area(
        container(
            container(Space::new())
                .width(Length::Fixed(54.0))
                .height(Length::Fixed(2.0))
                .style(resize_rail_style),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fixed(10.0)),
    )
    .on_press(Message::StartUiResize(crate::UiResizeTarget::Composer))
    .on_release(Message::StopUiResize)
    .interaction(mouse::Interaction::ResizingVertically)
    .into()
}

fn danger_zone_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(34, 22, 30)
        } else {
            rgb(255, 242, 245)
        })),
        border: Border {
            color: if is_dark_mode() {
                rgb(118, 56, 74)
            } else {
                rgb(220, 155, 170)
            },
            width: 1.0,
            radius: Radius::from(18.0),
        },
        shadow: Shadow::default(),
    }
}

fn brighten(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r - amount).max(0.0),
        g: (color.g - amount).max(0.0),
        b: (color.b - amount).max(0.0),
        a: color.a,
    }
}

fn button_visual(
    background: Color,
    border: Color,
    text: Color,
    status: widget::button::Status,
) -> widget::button::Style {
    let (background, border, offset_y, blur_radius) = match status {
        widget::button::Status::Hovered => (
            brighten(background, 0.035),
            brighten(border, 0.045),
            3.0,
            14.0,
        ),
        widget::button::Status::Pressed => {
            (darken(background, 0.045), brighten(border, 0.025), 0.0, 3.0)
        }
        widget::button::Status::Disabled => {
            (darken(background, 0.055), darken(border, 0.055), 0.0, 0.0)
        }
        _ => (background, border, 1.0, 4.0),
    };

    widget::button::Style {
        snap: true,
        background: Some(Background::Color(background)),
        text_color: text,
        border: Border {
            color: border,
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow {
            color: shadow_color(),
            offset: Vector::from([0.0, offset_y]),
            blur_radius,
        },
    }
}

fn primary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(14).align_x(Horizontal::Center))
        .padding([12, 16])
        .style(|_theme, status| {
            button_visual(
                if is_dark_mode() {
                    rgb(111, 91, 218)
                } else {
                    rgb(112, 91, 218)
                },
                accent(),
                Color::WHITE,
                status,
            )
        })
        .on_press(message)
        .into()
}

fn secondary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(14).align_x(Horizontal::Center))
        .padding([11, 14])
        .style(|_theme, _status| button_visual(panel_soft(), border_soft(), text_main(), _status))
        .on_press(message)
        .into()
}

fn danger_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(14).align_x(Horizontal::Center))
        .padding(12)
        .style(|_theme, _status| {
            button_visual(rgb(104, 38, 55), rgb(185, 76, 99), Color::WHITE, _status)
        })
        .on_press(message)
        .into()
}

fn mini_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([8, 10])
        .style(|_theme, _status| button_visual(panel_soft(), border_soft(), text_muted(), _status))
        .on_press(message)
        .into()
}

fn mini_button_owned(label: String, message: Message) -> Element<'static, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([8, 10])
        .style(|_theme, _status| button_visual(panel_soft(), border_soft(), text_muted(), _status))
        .on_press(message)
        .into()
}

fn settings_disclosure_button<'a>(
    label: &'a str,
    open: bool,
    message: Message,
) -> Element<'a, Message> {
    widget::button(
        widget::row![
            widget::text(label).size(12).color(text_main()),
            Space::new().width(Length::Fill),
            widget::text(if open { "▾" } else { "▸" })
                .size(13)
                .color(accent_2()),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 12])
    .width(Length::Fill)
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_main(), status))
    .on_press(message)
    .into()
}

fn toolbar_button<'a>(icon: &'a str, label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(
        widget::row![
            widget::text(icon)
                .size(14)
                .color(accent_2())
                .align_x(Horizontal::Center),
            Space::new().width(Length::Fixed(7.0)),
            widget::text(label).size(13).color(text_main()),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 13])
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_main(), status))
    .on_press(message)
    .into()
}

fn suggestion_button(
    emoji: &'static str,
    label: &'static str,
    message: Message,
) -> Element<'static, Message> {
    widget::button(
        widget::row![
            widget::text(emoji)
                .size(18)
                .width(Length::Fixed(28.0))
                .align_x(Horizontal::Center),
            widget::text(label)
                .size(13)
                .color(text_main())
                .width(Length::Fill),
            widget::text("↗").size(14).color(accent_2()),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([13, 15])
    .width(Length::Fill)
    .height(Length::Fixed(58.0))
    .style(|_theme, status| {
        let background = match status {
            widget::button::Status::Hovered => {
                if is_dark_mode() {
                    rgb(32, 35, 51)
                } else {
                    rgb(246, 244, 253)
                }
            }
            _ => panel_soft(),
        };
        let border = if matches!(status, widget::button::Status::Hovered) {
            accent()
        } else {
            border_soft()
        };
        button_visual(background, border, text_main(), status)
    })
    .on_press(message)
    .into()
}

fn suggestion_grid(
    labels: [&'static str; 4],
    prompts: [&'static str; 4],
) -> Element<'static, Message> {
    const EMOJIS: [&str; 4] = ["🧠", "🗺️", "💻", "💡"];

    widget::responsive(move |size| {
        let suggestion = |index: usize| {
            suggestion_button(
                EMOJIS[index],
                labels[index],
                Message::UseSuggestion(prompts[index].to_string()),
            )
        };

        if size.width < 480.0 {
            widget::column![suggestion(0), suggestion(1), suggestion(2), suggestion(3)]
                .spacing(iced::Pixels(10.0))
                .width(Length::Fill)
                .into()
        } else {
            widget::column![
                widget::row![suggestion(0), suggestion(1)]
                    .spacing(iced::Pixels(10.0))
                    .width(Length::Fill),
                widget::row![suggestion(2), suggestion(3)]
                    .spacing(iced::Pixels(10.0))
                    .width(Length::Fill),
            ]
            .spacing(iced::Pixels(10.0))
            .width(Length::Fill)
            .into()
        }
    })
    .height(Length::Shrink)
    .into()
}

fn send_button<'a>(label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    widget::button(
        widget::row![
            widget::text(label).size(14),
            Space::new().width(Length::Fixed(7.0)),
            widget::text("↑").size(18),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([12, 16])
    .style(|_theme, status| {
        button_visual(
            if is_dark_mode() {
                rgb(111, 91, 218)
            } else {
                rgb(112, 91, 218)
            },
            accent(),
            Color::WHITE,
            status,
        )
    })
    .on_press_maybe(message)
    .into()
}

fn assistant_mark_style(pulse: f32, active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(brighten(
            if is_dark_mode() {
                rgb(104, 88, 205)
            } else {
                rgb(117, 98, 221)
            },
            if active { pulse * 0.018 } else { 0.0 },
        ))),
        border: Border {
            color: brighten(accent(), if active { pulse * 0.018 } else { 0.0 }),
            width: if active { 1.0 + pulse * 0.18 } else { 1.0 },
            radius: Radius::from(14.0),
        },
        shadow: Shadow {
            color: with_alpha(accent(), if active { 0.20 + pulse * 0.08 } else { 0.20 }),
            offset: Vector::from([0.0, if active { 4.0 + pulse } else { 4.0 }]),
            blur_radius: if active { 14.0 + pulse * 4.0 } else { 16.0 },
        },
    }
}

fn thinking_control<'a>(
    selected: ThinkingLevel,
    levels: &[ThinkingLevel],
    language: Language,
) -> Element<'a, Message> {
    let description = match selected {
        ThinkingLevel::Off => "Fastest · no extra reasoning",
        ThinkingLevel::On => "Use this model's standard reasoning mode",
        ThinkingLevel::Minimal => "Minimal reasoning for very quick responses",
        ThinkingLevel::Low => "Quick reasoning for everyday questions",
        ThinkingLevel::Medium => "Balanced for multi-step tasks",
        ThinkingLevel::High => "Most thorough · slower responses",
        ThinkingLevel::XHigh => "Extra-deep reasoning for difficult tasks",
        ThinkingLevel::Max => "Maximum reasoning the model offers",
    };

    widget::column![
        widget::pick_list(
            ThinkingChoice::from_levels(levels, language),
            Some(ThinkingChoice {
                level: selected,
                language,
            }),
            |choice| Message::ThinkingLevelChange(choice.level),
        )
        .placeholder(tr(language, "Thinking"))
        .padding([12, 14])
        .text_size(14)
        .style(pick_list_style)
        .menu_style(pick_list_menu_style)
        .width(Length::Fill),
        Space::new().height(Length::Fixed(6.0)),
        widget::text(tr(language, description))
            .size(11)
            .color(text_faint()),
    ]
    .into()
}

fn image_preview<'a>(
    image: &ChatImage,
    remove_index: Option<usize>,
    language: Language,
) -> Element<'a, Message> {
    let preview = widget::image(image.preview_handle.clone())
        .width(Length::Fixed(160.0))
        .height(Length::Fixed(110.0))
        .content_fit(iced::ContentFit::Contain)
        .border_radius(10.0);
    let footer: Element<Message> = if let Some(index) = remove_index {
        widget::row![
            widget::text(ellipsize_chat_title(&image.name, 16))
                .size(11)
                .color(text_muted())
                .wrapping(Wrapping::None),
            Space::new().width(Length::Fill),
            mini_button(tr(language, "Remove"), Message::RemoveImage(index)),
        ]
        .into()
    } else {
        widget::text(ellipsize_chat_title(
            &format!("{} · {}", image.name, image.mime_type),
            22,
        ))
        .size(11)
        .color(text_muted())
        .wrapping(Wrapping::None)
        .into()
    };
    container(widget::column![
        preview,
        Space::new().height(Length::Fixed(6.0)),
        footer
    ])
    .padding(8)
    .width(Length::Fixed(176.0))
    .clip(true)
    .style(flat_card_style)
    .into()
}

fn image_previews<'a>(
    images: &[ChatImage],
    removable: bool,
    language: Language,
) -> Element<'a, Message> {
    if images.is_empty() {
        return widget::column![].into();
    }

    let previews = widget::Row::with_children(
        images
            .iter()
            .enumerate()
            .map(|(index, image)| image_preview(image, removable.then_some(index), language))
            .collect::<Vec<_>>(),
    )
    .spacing(iced::Pixels(6.0));

    widget::scrollable(previews)
        .direction(widget::scrollable::Direction::Horizontal(
            widget::scrollable::Scrollbar::default()
                .width(5.0)
                .scroller_width(5.0),
        ))
        .height(Length::Fixed(172.0))
        .into()
}

fn composer_image_preview<'a>(image: &ChatImage, index: usize) -> Element<'a, Message> {
    let preview = widget::image(image.preview_handle.clone())
        .width(Length::Fixed(46.0))
        .height(Length::Fixed(46.0))
        .content_fit(iced::ContentFit::Cover)
        .border_radius(8.0);
    let remove = widget::button(
        widget::text("×")
            .size(14)
            .color(text_muted())
            .align_x(Horizontal::Center),
    )
    .padding([3, 6])
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_muted(), status))
    .on_press(Message::RemoveImage(index));

    container(
        widget::row![preview, Space::new().width(Length::Fixed(4.0)), remove,]
            .align_y(iced::Alignment::Center),
    )
    .padding(5)
    .width(Length::Fixed(88.0))
    .height(Length::Fixed(56.0))
    .style(flat_card_style)
    .into()
}

fn composer_image_previews<'a>(images: &[ChatImage]) -> Element<'a, Message> {
    let previews = widget::Row::with_children(
        images
            .iter()
            .enumerate()
            .map(|(index, image)| composer_image_preview(image, index))
            .collect::<Vec<_>>(),
    )
    .spacing(iced::Pixels(6.0));
    let rail_width = (images.len() as f32 * 94.0).min(212.0);

    widget::scrollable(previews)
        .direction(widget::scrollable::Direction::Horizontal(
            widget::scrollable::Scrollbar::default()
                .width(4.0)
                .scroller_width(4.0),
        ))
        .width(Length::Fixed(rail_width))
        .height(Length::Fixed(62.0))
        .into()
}

fn copy_code_button<'a>(code: String, copied: bool, language: Language) -> Element<'a, Message> {
    let label = tr(language, if copied { "Copied ✓" } else { "Copy code" });

    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding(8)
        .style(move |_theme, status| {
            if copied {
                button_visual(rgb(31, 92, 63), rgb(93, 225, 144), Color::WHITE, status)
            } else {
                button_visual(panel_soft(), border_soft(), text_muted(), status)
            }
        })
        .on_press(Message::CopyPressed(code))
        .into()
}

fn text_input_style(
    _theme: &Theme,
    status: widget::text_input::Status,
) -> widget::text_input::Style {
    let focused = matches!(status, widget::text_input::Status::Focused { .. });
    let hovered = matches!(status, widget::text_input::Status::Hovered);
    widget::text_input::Style {
        background: Background::Color(panel_soft()),
        border: Border {
            color: if focused {
                accent()
            } else if hovered {
                border_bright()
            } else {
                border_soft()
            },
            width: if focused { 1.5 } else { 1.0 },
            radius: Radius::from(12.0),
        },
        icon: text_muted(),
        placeholder: text_faint(),
        value: text_main(),
        selection: accent(),
    }
}

fn feedback_text_input_style(
    bounce: f32,
) -> impl Fn(&Theme, widget::text_input::Status) -> widget::text_input::Style {
    move |theme, status| {
        let mut style = text_input_style(theme, status);
        style.background = Background::Color(mix_color(panel_soft(), accent_2(), bounce * 0.05));
        style.border.color = mix_color(style.border.color, accent_2(), bounce * 0.35);
        style.border.width += bounce * 0.55;
        style
    }
}

fn text_editor_style(
    _theme: &Theme,
    status: widget::text_editor::Status,
) -> widget::text_editor::Style {
    let focused = matches!(status, widget::text_editor::Status::Focused { .. });
    let hovered = matches!(status, widget::text_editor::Status::Hovered);
    widget::text_editor::Style {
        background: Background::Color(panel_soft()),
        border: Border {
            color: if focused {
                accent()
            } else if hovered {
                border_bright()
            } else {
                border_soft()
            },
            width: if focused { 1.5 } else { 1.0 },
            radius: Radius::from(12.0),
        },
        placeholder: text_faint(),
        value: text_main(),
        selection: accent(),
    }
}

fn section_title<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    widget::column![
        widget::text(title).size(27).color(text_main()),
        Space::new().height(Length::Fixed(5.0)),
        widget::text(subtitle).size(14).color(text_muted()),
    ]
    .into()
}

fn setting_label<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    widget::column![
        widget::text(title).size(16).color(text_main()),
        Space::new().height(Length::Fixed(4.0)),
        widget::text(subtitle).size(12).color(text_muted()),
    ]
    .width(Length::Fill)
    .into()
}

fn settings_group_title<'a>(title: &'a str) -> Element<'a, Message> {
    widget::row![
        widget::text(title).size(11).color(accent_2()),
        Space::new().width(Length::Fixed(10.0)),
        widget::rule::horizontal(1).style(|_theme| widget::rule::Style {
            color: border_soft(),
            radius: Radius::from(1.0),
            fill_mode: widget::rule::FillMode::Full,
            snap: true,
        }),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn help_card<'a>(title: &'a str, body: &'a str, color: Color) -> Element<'a, Message> {
    container(widget::column![
        container(widget::text(title).size(16).color(text_main()))
            .padding(8)
            .style(chip_style(color)),
        Space::new().height(Length::Fixed(10.0)),
        widget::text(body).size(14).color(text_muted()),
    ])
    .padding(16)
    .width(Length::Fill)
    .style(flat_card_style)
    .into()
}

fn website_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .map(|host| host.strip_prefix("www.").unwrap_or(&host).to_string())
        .unwrap_or_else(|| url.to_string())
}

fn ellipsize_chat_title(title: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let single_line = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single_line.chars();
    let mut shortened = chars.by_ref().take(max_chars).collect::<String>();

    if chars.next().is_some() {
        shortened.pop();
        shortened.push('…');
    }

    shortened
}

fn website_result_row(
    index: usize,
    source: &WebSource,
    active: bool,
    language: Language,
) -> Element<'static, Message> {
    let host = website_host(&source.url);
    let title = ellipsize_chat_title(&source.title, 64);
    let host = ellipsize_chat_title(&host, 64);
    let trailing = if active {
        tr(language, "READING").to_string()
    } else {
        "↗".to_string()
    };
    let trailing_color = if active { accent_2() } else { text_faint() };

    container(
        widget::button(widget::row![
            container(
                widget::text((index + 1).to_string())
                    .size(11)
                    .color(if active { accent_2() } else { text_muted() })
                    .align_x(Horizontal::Center)
            )
            .padding([4, 7])
            .style(chip_style(if active {
                accent_2()
            } else {
                border_bright()
            })),
            Space::new().width(Length::Fixed(9.0)),
            widget::column![
                widget::text(title)
                    .size(12)
                    .color(text_main())
                    .wrapping(Wrapping::None),
                Space::new().height(Length::Fixed(2.0)),
                widget::text(host)
                    .size(11)
                    .color(text_muted())
                    .wrapping(Wrapping::None),
            ]
            .width(Length::Fill),
            widget::text(trailing).size(10).color(trailing_color),
        ])
        .on_press(Message::OpenSource(source.url.clone()))
        .padding(0)
        .style(chat_title_button_style)
        .clip(true)
        .width(Length::Fill),
    )
    .padding(9)
    .width(Length::Fill)
    .style(website_row_style(active))
    .into()
}

fn web_search_activity<'a>(state: WebSearchState, language: Language) -> Element<'a, Message> {
    let (status, detail, websites, active_url, status_color) = match state {
        WebSearchState::Searching { query, websites } => (
            tr(language, "Searching the web…"),
            query,
            websites,
            None,
            accent_2(),
        ),
        WebSearchState::Results { query, websites } => (
            tr(language, "Reviewing results"),
            query,
            websites,
            None,
            warning(),
        ),
        WebSearchState::Fetching {
            url,
            query,
            websites,
        } => (
            tr(language, "Reading website"),
            if query.trim().is_empty() {
                website_host(&url)
            } else {
                query
            },
            websites,
            Some(url),
            success(),
        ),
        WebSearchState::Synthesizing {
            query, websites, ..
        } => (
            tr(language, "Preparing the answer from these sources."),
            if query.trim().is_empty() {
                tr(language, "The model is choosing which result to read.").to_string()
            } else {
                query
            },
            websites,
            None,
            accent_2(),
        ),
        WebSearchState::Failed { message } => (
            tr(language, "Web search"),
            message,
            Vec::new(),
            None,
            danger(),
        ),
        WebSearchState::Idle | WebSearchState::Completed => {
            return widget::column![].into();
        }
    };
    let detail = ellipsize_chat_title(&detail, 72);
    let result_count: Element<'a, Message> = if websites.is_empty() {
        widget::column![].into()
    } else {
        let count = websites.len();
        container(
            widget::text(format!("{count} {}", tr(language, "found")))
                .size(10)
                .color(status_color),
        )
        .padding([4, 7])
        .style(chip_style(status_color))
        .into()
    };
    let header: Element<'a, Message> = container(widget::row![
        widget::text("●").size(10).color(status_color),
        Space::new().width(Length::Fixed(7.0)),
        widget::text(status)
            .size(12)
            .color(status_color)
            .wrapping(Wrapping::None),
        Space::new().width(Length::Fixed(9.0)),
        widget::text(detail)
            .size(12)
            .color(text_muted())
            .wrapping(Wrapping::None)
            .width(Length::Fill),
        result_count,
    ])
    .padding([8, 10])
    .width(Length::Fill)
    .clip(true)
    .style(web_activity_style)
    .into();
    let result_rows = websites
        .iter()
        .enumerate()
        .take(10)
        .map(|(index, source)| {
            website_result_row(
                index,
                source,
                active_url.as_deref() == Some(source.url.as_str()),
                language,
            )
        })
        .collect::<Vec<_>>();
    let results: Element<'a, Message> = if result_rows.is_empty() {
        widget::column![].into()
    } else {
        container(widget::Column::with_children(result_rows).spacing(iced::Pixels(4.0)))
            .padding([6, 0])
            .width(Length::Fill)
            .into()
    };

    widget::column![header, results,].into()
}

fn markdown_with_code_copy<'a>(
    items: &'a [markdown::Item],
    text_size: f32,
    copied_text: Option<&String>,
    language: Language,
    code_checking_enabled: bool,
    markdown_images: &'a std::collections::HashMap<String, MarkdownImageState>,
    motion: f32,
) -> Element<'a, Message> {
    let settings = iced::widget::markdown::Settings::with_text_size(
        text_size,
        if is_dark_mode() {
            Theme::Dark
        } else {
            Theme::Light
        },
    );

    let mut children: Vec<Element<'a, Message>> = Vec::new();

    for item in items.iter() {
        if let markdown::Item::Image { url, .. } = item {
            let image_block: Element<'a, Message> = match markdown_images.get(url) {
                Some(MarkdownImageState::Ready(handle)) => container(widget::column![
                    widget::image(handle.clone())
                        .width(Length::Fill)
                        .height(Length::Fixed(320.0))
                        .content_fit(iced::ContentFit::Contain)
                        .border_radius(12.0),
                    Space::new().height(Length::Fixed(7.0)),
                    widget::text(ellipsize_chat_title(&website_host(url), 64))
                        .size(11)
                        .color(text_muted())
                        .wrapping(Wrapping::None),
                ])
                .padding(10)
                .width(Length::Fill)
                .max_width(720)
                .style(flat_card_style)
                .into(),
                Some(MarkdownImageState::Failed(error)) => container(widget::row![
                    widget::text("!")
                        .size(12)
                        .color(danger())
                        .align_x(Horizontal::Center),
                    Space::new().width(Length::Fixed(8.0)),
                    widget::column![
                        widget::text("Image unavailable")
                            .size(12)
                            .color(text_main()),
                        widget::text(ellipsize_chat_title(error, 88))
                            .size(11)
                            .color(text_muted()),
                    ],
                ])
                .padding([10, 12])
                .width(Length::Fill)
                .style(chip_style(danger()))
                .into(),
                Some(MarkdownImageState::Loading) | None => {
                    let dots = ".".repeat(((motion * 4.0) as usize % 3) + 1);
                    container(widget::row![
                        widget::text("○").size(13).color(accent_2()),
                        Space::new().width(Length::Fixed(8.0)),
                        widget::text(format!("Loading image{dots}"))
                            .size(12)
                            .color(text_muted()),
                    ])
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(chip_style(accent_2()))
                    .into()
                }
            };
            children.push(image_block);
            continue;
        }

        children.push(selectable_markdown(std::iter::once(item), settings).map(|_| Message::None));

        if let markdown::Item::CodeBlock {
            language: code_language,
            code,
            ..
        } = item
        {
            let copied = copied_text.map(|copied| copied == code).unwrap_or(false);
            let check_button: Element<'a, Message> = code_language
                .as_deref()
                .and_then(crate::canonical_code_language)
                .filter(|_| code_checking_enabled)
                .map(|canonical| {
                    mini_button_owned(
                        tr(language, "Check code").to_string(),
                        Message::CheckCode(canonical.to_string(), code.clone()),
                    )
                })
                .unwrap_or_else(|| widget::column![].into());

            children.push(
                widget::row![
                    Space::new().width(Length::Fill),
                    check_button,
                    Space::new().width(Length::Fixed(6.0)),
                    copy_code_button(code.clone(), copied, language),
                ]
                .into(),
            );
        }
    }

    widget::Column::with_children(children)
        .spacing(iced::Pixels(8.0))
        .into()
}

#[allow(clippy::too_many_arguments)]
fn message_bubble<'a>(
    index: usize,
    message: &'a Correspondence,
    parsed_markdown: Option<&'a [markdown::Item]>,
    cached_thinking: &'a str,
    text_size: f32,
    model_name: String,
    copied_text: Option<&String>,
    thinking_expanded: bool,
    sources_expanded: bool,
    language: Language,
    code_checking_enabled: bool,
    show_tokens_per_second: bool,
    markdown_images: &'a std::collections::HashMap<String, MarkdownImageState>,
    reveal: f32,
    motion: f32,
) -> Element<'a, Message> {
    match message {
        Correspondence::User { text, images } => widget::row![
            Space::new().width(Length::Fill),
            container(widget::column![
                widget::text(tr(language, "You"))
                    .size(12)
                    .color(if is_dark_mode() {
                        rgb(205, 221, 255)
                    } else {
                        rgb(55, 72, 150)
                    })
                    .align_x(Horizontal::Right),
                Space::new().height(Length::Fixed(6.0)),
                image_previews(images, false, language),
                widget::text(text)
                    .size(text_size)
                    .align_x(Horizontal::Right),
            ])
            .padding([13, 16])
            .width(Length::Shrink)
            .max_width(760)
            .style(user_bubble_style(reveal)),
            Space::new().width(Length::Fixed(4.0 + (1.0 - reveal) * 8.0)),
        ]
        .into(),

        Correspondence::Bot {
            text,
            thinking_seconds,
            tokens_per_second,
            sources,
            web_search_used,
            ..
        } => {
            let body: Element<'a, Message> = if let Some(parsed) = parsed_markdown {
                markdown_with_code_copy(
                    parsed,
                    text_size,
                    copied_text,
                    language,
                    code_checking_enabled,
                    markdown_images,
                    motion,
                )
            } else {
                let (_, fallback_text) = split_thinking_text(text);
                widget::text(fallback_text)
                    .size(text_size)
                    .color(text_main())
                    .align_x(Horizontal::Left)
                    .into()
            };

            let reasoning: Element<'a, Message> = if cached_thinking.is_empty() {
                widget::column![].into()
            } else {
                let label = if let Some(seconds) = thinking_seconds {
                    if thinking_expanded {
                        format!(
                            "▾ {}",
                            if language == Language::Spanish {
                                format!("Razonó durante {seconds} segundos")
                            } else {
                                format!("Thought for {seconds} seconds")
                            }
                        )
                    } else if language == Language::Spanish {
                        format!("▸ Razonó durante {seconds} segundos")
                    } else {
                        format!("▸ Thought for {seconds} seconds")
                    }
                } else if thinking_expanded {
                    tr(language, "▾ Hide thinking").to_string()
                } else {
                    tr(language, "▸ Show thinking").to_string()
                };
                let details: Element<'a, Message> = if thinking_expanded {
                    container(
                        widget::text(cached_thinking)
                            .size(text_size - 1.0)
                            .color(text_muted()),
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                } else {
                    widget::column![].into()
                };
                widget::column![
                    mini_button_owned(label, Message::ToggleThinking(index)),
                    details,
                    Space::new().height(Length::Fixed(7.0)),
                ]
                .into()
            };

            let source_list: Element<'a, Message> = if sources.is_empty() {
                widget::column![].into()
            } else {
                let label = if sources_expanded {
                    format!("▾ {} ({})", tr(language, "Sources"), sources.len())
                } else {
                    format!("▸ {} ({})", tr(language, "Sources"), sources.len())
                };
                let entries: Element<'a, Message> = if sources_expanded {
                    let entries = sources
                        .iter()
                        .enumerate()
                        .map(|(source_index, source)| {
                            website_result_row(source_index, source, false, language)
                        })
                        .collect::<Vec<Element<'a, Message>>>();
                    widget::column![
                        Space::new().height(Length::Fixed(5.0)),
                        widget::Column::with_children(entries).spacing(iced::Pixels(5.0)),
                    ]
                    .into()
                } else {
                    widget::column![].into()
                };
                widget::column![
                    Space::new().height(Length::Fixed(12.0)),
                    mini_button_owned(label, Message::ToggleSources(index)),
                    entries,
                ]
                .into()
            };

            // Generation speed shown at the bottom of the reply when the
            // setting is enabled. The value comes straight from Ollama's
            // statistics, so render batching never distorts it.
            let speed_note: Element<'a, Message> = if show_tokens_per_second {
                match tokens_per_second {
                    Some(tps) => widget::column![
                        Space::new().height(Length::Fixed(10.0)),
                        widget::text(format!("{tps:.1} tokens/s"))
                            .size(11)
                            .color(text_muted()),
                    ]
                    .into(),
                    None => widget::column![].into(),
                }
            } else {
                widget::column![].into()
            };

            widget::row![
                Space::new().width(Length::Fixed((1.0 - reveal) * 8.0)),
                container(widget::text("✦").size(17).color(Color::WHITE))
                    .padding([8, 11])
                    .style(assistant_mark_style(motion, false)),
                Space::new().width(Length::Fixed(10.0)),
                container(widget::column![
                    widget::row![
                        widget::text(model_name).size(12).color(accent_2()),
                        Space::new().width(Length::Fill),
                        if *web_search_used {
                            container(widget::text(tr(language, "WEB")).size(10).color(success()))
                                .padding([4, 7])
                                .style(chip_style(success()))
                        } else {
                            container(widget::text("")).padding(0)
                        },
                    ],
                    Space::new().height(Length::Fixed(7.0)),
                    reasoning,
                    body,
                    source_list,
                    speed_note,
                ])
                .padding(14)
                .width(Length::Fill)
                .max_width(920)
                .style(bot_bubble_style(reveal)),
                Space::new().width(Length::Fixed(24.0)),
            ]
            .into()
        }
    }
}

impl Program {
    pub fn get_ui_information<'a>(
        &'a self,
        gui_state: &'a GUIState,
    ) -> iced::widget::Container<'a, Message> {
        let language = self.user_information.language;
        match gui_state {
            GUIState::InfoPopup => {
                let content = container(
                    widget::column![
                        Space::new().height(Length::Fixed(
                            (1.0 - eased(self.page_reveal)) * 4.0
                        )),
                        container(
                            widget::row![
                                widget::column![
                                    widget::text("Locoryn")
                                        .size(30)
                                        .color(text_main()),
                                    Space::new().height(Length::Fixed(6.0)),
                                    widget::text(tr(language, "A polished desktop interface for chatting with local Ollama models."))
                                        .size(14)
                                        .color(text_muted()),
                                ]
                                .width(Length::Fill),

                                container(
                                    widget::text(tr(language, "HELP"))
                                        .size(13)
                                        .color(text_main())
                                )
                                .padding(10)
                                .style(chip_style(accent_2())),
                            ]
                        )
                        .padding(20)
                        .width(Length::Fill)
                        .style(top_bar_style),

                        Space::new().height(Length::Fixed(14.0)),

                        container(
                            widget::column![
                                widget::row![
                                    help_card(
                                        tr(language, "Chat locally"),
                                        tr(language, "Select one of your installed Ollama models, type a prompt, and press Enter to generate a response."),
                                        accent(),
                                    ),
                                    Space::new().width(Length::Fixed(12.0)),
                                    help_card(
                                        tr(language, "Manage models"),
                                        tr(language, "Use Advanced Settings to install models by name, change the Ollama address, or tune response rendering."),
                                        accent_2(),
                                    ),
                                ],

                                Space::new().height(Length::Fixed(12.0)),

                                widget::row![
                                    help_card(
                                        tr(language, "System prompts"),
                                        tr(language, "System prompts let you switch the assistant's behaviour or personality without rewriting your prompt each time."),
                                        warning(),
                                    ),
                                    Space::new().width(Length::Fixed(12.0)),
                                    help_card(
                                        tr(language, "Chat history"),
                                        tr(language, "When enabled, conversations can be saved locally. You can wipe the current history from Settings."),
                                        danger(),
                                    ),
                                ],

                                Space::new().height(Length::Fixed(16.0)),

                                container(
                                    widget::column![
                                    widget::text(tr(language, "Files and configuration"))
                                            .size(17)
                                            .color(text_main()),
                                        Space::new().height(Length::Fixed(8.0)),
                                        widget::text(
                                            tr(language, "User settings, logs, generated images, and chats are stored in your local application-data folder. Installed assets remain read-only.")
                                        )
                                        .size(14)
                                        .color(text_muted()),
                                    ]
                                )
                                .padding(16)
                                .width(Length::Fill)
                                .style(flat_card_style),
                            ]
                        )
                        .padding(18)
                        .width(Length::Fill)
                        .style(conversation_style),

                        Space::new().height(Length::Fixed(14.0)),

                        widget::row![
                            Space::new().width(Length::Fill),
                            primary_button(tr(language, "Back to chat"), Message::ToggleInfoPopup),
                        ],
                    ]
                )
                .padding(0)
                .width(Length::Fill);

                container(widget::scrollable(content).height(Length::Fill))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::Main => {
                let user_information = self.user_information.clone();
                let bots_list = self.app_state.bots_list.lock().unwrap().clone();
                let copied_text = self.last_copied_text.clone();
                let local_ollamastate = self.app_state.ollama_state.lock().unwrap().clone();
                let active_prompt = self.current_active_prompt();
                let is_processing = active_prompt.is_some();
                let current_web_search_enabled = active_prompt
                    .map(|job| job.web_search_enabled)
                    .unwrap_or(self.web_search_for_chat);
                let web_search_state = active_prompt
                    .map(|job| job.web_search_state.clone())
                    .unwrap_or(WebSearchState::Idle);
                let web_synthesis_thinking = active_prompt
                    .and_then(|job| match &job.web_search_state {
                        WebSearchState::Synthesizing { thinking, .. } => Some(thinking.as_str()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let live_thinking = active_prompt
                    .map(|job| job.thinking_text.as_str())
                    .unwrap_or_default();
                let live_thinking = if live_thinking.is_empty() {
                    web_synthesis_thinking
                } else {
                    live_thinking
                };

                let chat_messages = &self.chat_messages_cache;
                let chat_is_empty = chat_messages.is_empty();

                let selected_model = self.user_information.model.clone();
                let active_model_name = selected_model
                    .clone()
                    .unwrap_or_else(|| tr(language, "No model selected").to_string());
                let current_chat_title = if self.temporary_chat {
                    tr(language, "Temporary chat").to_string()
                } else {
                    self.saved_chats
                        .iter()
                        .find(|chat| chat.id == self.current_chat_id)
                        .map(|chat| chat.title.clone())
                        .or_else(|| {
                            chat_messages.iter().find_map(|message| match message {
                                Correspondence::User { text, .. } => Some(text.clone()),
                                Correspondence::Bot { .. } => None,
                            })
                        })
                        .map(|title| ellipsize_chat_title(&title, 48))
                        .unwrap_or_else(|| tr(language, "New conversation").to_string())
                };

                let prompt_to_send = self.prompt.prompt.clone();
                let prompt = widget::text_editor(&self.prompt.editor)
                    .placeholder(tr(language, "Ask something..."))
                    .padding(14)
                    .size(18)
                    .height(Length::Fill)
                    .min_height(52)
                    .on_action(Message::EditPrompt)
                    .key_binding(move |key_press| {
                        if matches!(
                            key_press.key.as_ref(),
                            iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                        ) && !key_press.modifiers.shift()
                        {
                            Some(widget::text_editor::Binding::Custom(Message::Prompt(
                                prompt_to_send.clone(),
                            )))
                        } else {
                            widget::text_editor::Binding::from_key_press(key_press)
                        }
                    })
                    .style(text_editor_style);
                let web_toggle: Element<Message> = if is_processing {
                    container(
                        widget::text(if current_web_search_enabled {
                            tr(language, "Web on")
                        } else {
                            tr(language, "Web off")
                        })
                        .size(12)
                        .color(text_muted()),
                    )
                    .padding([7, 9])
                    .style(chip_style(text_muted()))
                    .into()
                } else {
                    mini_button(
                        if current_web_search_enabled {
                            tr(language, "Web on")
                        } else {
                            tr(language, "Web off")
                        },
                        Message::ToggleChatWebSearch,
                    )
                };

                let chat_widgets: Vec<Element<Message>> = chat_messages
                    .iter()
                    .enumerate()
                    .flat_map(|(index, message)| {
                        let parsed_markdown =
                            self.chat_markdown_cache.get(index).map(Vec::as_slice);
                        let cached_thinking = self
                            .chat_thinking_cache
                            .get(index)
                            .map(String::as_str)
                            .unwrap_or_default();
                        let message_model_name = self
                            .chat_model_name_cache
                            .get(index)
                            .cloned()
                            .flatten()
                            .unwrap_or_else(|| active_model_name.clone());
                        let reveal = eased(self.page_reveal * 1.3 - (index.min(10) as f32 * 0.025));
                        let motion = 1.0 - (self.ui_motion * 2.0 - 1.0).abs();

                        vec![
                            message_bubble(
                                index,
                                message,
                                parsed_markdown,
                                cached_thinking,
                                user_information.text_size,
                                message_model_name,
                                copied_text.as_ref(),
                                self.expanded_thinking.contains(&index),
                                self.expanded_sources.contains(&index),
                                language,
                                self.code_checking_enabled,
                                self.show_tokens_per_second,
                                &self.markdown_images,
                                reveal,
                                motion,
                            ),
                            Space::new().height(Length::Fixed(10.0)).into(),
                        ]
                    })
                    .collect();

                let online = local_ollamastate.to_lowercase() != "offline";
                let status_color = if online { success() } else { danger() };
                let pulse = 1.0 - (self.ui_motion * 2.0 - 1.0).abs();
                let visible_debug = self.current_debug_message().clone();
                let debug_color = if visible_debug.is_error {
                    danger()
                } else {
                    success()
                };

                let model_selector: Element<Message> = if bots_list.is_empty() {
                    container(
                        widget::text(tr(language, "No models installed"))
                            .size(13)
                            .color(text_muted()),
                    )
                    .padding(10)
                    .style(chip_style(danger()))
                    .into()
                } else {
                    widget::pick_list(
                        bots_list.clone(),
                        selected_model.clone(),
                        Message::ModelChange,
                    )
                    .padding([12, 14])
                    .text_size(14)
                    .style(pick_list_style)
                    .menu_style(pick_list_menu_style)
                    .width(Length::Fill)
                    .into()
                };

                let thinking_selector: Element<Message> = widget::pick_list(
                    ThinkingChoice::from_levels(&self.user_information.thinking_levels, language),
                    Some(ThinkingChoice {
                        level: self.user_information.thinking_level,
                        language,
                    }),
                    |choice| Message::ThinkingLevelChange(choice.level),
                )
                .placeholder(tr(language, "Thinking"))
                .padding([12, 14])
                .text_size(14)
                .style(pick_list_style)
                .menu_style(pick_list_menu_style)
                .width(Length::Fixed(132.0))
                .into();

                let live_response: Element<Message> = if let Some(active_prompt) = active_prompt {
                    let response_model_name = active_prompt.model_name.clone();
                    let elapsed_seconds = active_prompt.started_at.elapsed().as_secs();
                    let activity = match &web_search_state {
                        WebSearchState::Searching { .. } => tr(language, "Searching"),
                        WebSearchState::Results { .. } => tr(language, "Reviewing results"),
                        WebSearchState::Fetching { .. } => tr(language, "Reading website"),
                        WebSearchState::Failed { .. } => tr(language, "Web search"),
                        WebSearchState::Idle
                        | WebSearchState::Synthesizing { .. }
                        | WebSearchState::Completed => {
                            if language == Language::Spanish {
                                "Pensando"
                            } else {
                                "Thinking"
                            }
                        }
                    };
                    let activity_dots = ".".repeat(((self.ui_motion * 4.0) as usize % 3) + 1);
                    let label = ellipsize_chat_title(
                        &format!(
                            "{response_model_name} · {activity}{activity_dots} ({elapsed_seconds}s)"
                        ),
                        64,
                    );
                    let web_search_activity_visible = !matches!(
                        &web_search_state,
                        WebSearchState::Idle | WebSearchState::Completed
                    );
                    let search_activity = web_search_activity(web_search_state.clone(), language);
                    let search_gap: Element<Message> = if web_search_activity_visible {
                        Space::new().height(Length::Fixed(8.0)).into()
                    } else {
                        widget::column![].into()
                    };

                    let live_reasoning: Element<Message> = if live_thinking.is_empty() {
                        widget::column![].into()
                    } else {
                        let expanded = self.expanded_thinking.contains(&usize::MAX);
                        let details: Element<Message> = if expanded {
                            container(
                                widget::text(live_thinking)
                                    .size(user_information.text_size - 1.0)
                                    .color(text_muted()),
                            )
                            .padding(12)
                            .width(Length::Fill)
                            .style(flat_card_style)
                            .into()
                        } else {
                            widget::column![].into()
                        };
                        widget::column![
                            mini_button(
                                tr(
                                    language,
                                    if expanded {
                                        "▾ Hide thinking"
                                    } else {
                                        "▸ Show thinking"
                                    }
                                ),
                                Message::ToggleThinking(usize::MAX),
                            ),
                            details,
                            Space::new().height(Length::Fixed(7.0)),
                        ]
                        .into()
                    };

                    widget::row![
                        container(widget::text("✦").size(17).color(Color::WHITE))
                            .padding([8, 11])
                            .style(assistant_mark_style(pulse, true)),
                        Space::new().width(Length::Fixed(10.0)),
                        container(widget::column![
                            widget::row![
                                container(
                                    widget::text(label)
                                        .size(12)
                                        .color(accent_2())
                                        .wrapping(Wrapping::None)
                                )
                                .width(Length::Fill)
                                .clip(true),
                                Space::new().width(Length::Fixed(10.0)),
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(96.0))
                                    .girth(Length::Fixed(5.0)),
                            ],
                            Space::new().height(Length::Fixed(8.0)),
                            search_activity,
                            search_gap,
                            live_reasoning,
                            markdown_with_code_copy(
                                &active_prompt.parsed_markdown,
                                user_information.text_size,
                                copied_text.as_ref(),
                                language,
                                self.code_checking_enabled,
                                &self.markdown_images,
                                self.ui_motion,
                            ),
                        ])
                        .padding(14)
                        .width(Length::Fill)
                        .max_width(920)
                        .style(bot_bubble_style(1.0)),
                        Space::new().width(Length::Fixed(24.0)),
                    ]
                    .into()
                } else if chat_is_empty {
                    let (suggestion_labels, suggestion_prompts) = if language == Language::Spanish {
                        (
                            [
                                "Explicar un tema complejo",
                                "Planificar un proyecto",
                                "Escribir o mejorar código",
                                "Generar ideas",
                            ],
                            [
                                "Explica un tema complejo de forma sencilla, con una analogía y un ejemplo práctico.",
                                "Ayúdame a convertir una idea en un plan claro con próximos pasos y posibles riesgos.",
                                "Ayúdame a escribir o mejorar código. Primero pregúntame por el lenguaje y el objetivo.",
                                "Genera varias ideas creativas, compáralas brevemente y recomienda las tres mejores.",
                            ],
                        )
                    } else {
                        (
                            [
                                "Explain a complex topic",
                                "Plan a project",
                                "Write or improve code",
                                "Brainstorm ideas",
                            ],
                            [
                                "Explain a complex topic simply, using an analogy and one practical example.",
                                "Help me turn an idea into a clear plan with next steps and likely risks.",
                                "Help me write or improve some code. Start by asking for the language and goal.",
                                "Brainstorm several creative ideas, compare them briefly, and recommend the best three.",
                            ],
                        )
                    };
                    widget::row![
                        Space::new().width(Length::Fill),
                        container(
                            widget::column![
                                container(widget::text("✦").size(28).color(Color::WHITE))
                                    .padding([
                                        (13.0 + pulse * 0.75) as u16,
                                        (17.0 + pulse * 0.75) as u16,
                                    ])
                                    .style(assistant_mark_style(pulse, true)),
                                Space::new().height(Length::Fixed(18.0)),
                                widget::text(tr(language, "What can I help you make?"))
                                    .size(28)
                                    .color(text_main())
                                    .align_x(Horizontal::Center),
                                Space::new().height(Length::Fixed(8.0)),
                                widget::text(tr(
                                    language,
                                    "Choose a starting point below, or write your own message."
                                ))
                                .size(14)
                                .color(text_muted())
                                .align_x(Horizontal::Center),
                                Space::new().height(Length::Fixed(22.0)),
                                suggestion_grid(suggestion_labels, suggestion_prompts),
                            ]
                            .align_x(Horizontal::Center)
                            .width(Length::Fill)
                        )
                        .padding([42, 24])
                        .width(Length::Fill)
                        .max_width(760),
                        Space::new().width(Length::Fill),
                    ]
                    .into()
                } else {
                    widget::column![].into()
                };

                let offline_hint: Element<Message> = if !online {
                    container(widget::row![
                        widget::column![
                            widget::text(tr(language, "Ollama was not detected."))
                                .size(14)
                                .color(text_main()),
                            Space::new().height(Length::Fixed(3.0)),
                            widget::text(tr(
                                language,
                                "Install Ollama or check your connection settings."
                            ))
                            .size(12)
                            .color(text_muted()),
                        ]
                        .width(Length::Fill),
                        secondary_button(
                            tr(language, "Install Ollama"),
                            Message::InstallationPrompt
                        ),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                } else {
                    widget::column![].into()
                };

                let missing_bots_hint: Element<Message> = if bots_list.is_empty() {
                    container(widget::row![
                        widget::column![
                            widget::text(tr(language, "No models were detected."))
                                .size(14)
                                .color(text_main()),
                            Space::new().height(Length::Fixed(3.0)),
                            widget::text(tr(language, "Install a model before sending prompts."))
                                .size(12)
                                .color(text_muted()),
                        ]
                        .width(Length::Fill),
                        secondary_button(tr(language, "Find models"), Message::ListPrompt),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                } else {
                    widget::column![].into()
                };

                let sidebar_width =
                    82.0 + self.sidebar_animation * (self.ui_layout.sidebar_width - 82.0);
                let show_sidebar_details = self.sidebar_animation > 0.52;
                let chat_sidebar: Element<Message> = if show_sidebar_details {
                    let mut entries: Vec<Element<Message>> = vec![
                        primary_button(tr(language, "＋ New chat"), Message::NewChat),
                        Space::new().height(Length::Fixed(6.0)).into(),
                        secondary_button(
                            if self.temporary_chat {
                                tr(language, "Leave temporary chat")
                            } else {
                                tr(language, "Temporary chat")
                            },
                            Message::ToggleTemporaryChat,
                        ),
                        Space::new().height(Length::Fixed(14.0)).into(),
                    ];
                    let has_temporary_chats = self.temporary_chat
                        || !self.temporary_chats.is_empty()
                        || self.active_prompts.values().any(|job| job.temporary);
                    if has_temporary_chats {
                        entries.push(
                            widget::text(tr(language, "Temporary chats"))
                                .size(13)
                                .color(text_muted())
                                .into(),
                        );
                    }
                    let mut temporary_jobs = self
                        .active_prompts
                        .iter()
                        .filter(|(_, job)| job.temporary && job.profile_id == self.active_profile_id)
                        .collect::<Vec<_>>();
                    temporary_jobs.sort_by_key(|(_, job)| job.started_at);
                    for (chat_id, job) in temporary_jobs {
                        let title = job
                            .chat_history
                            .lock()
                            .ok()
                            .and_then(|chat| {
                                chat.messages.iter().find_map(|message| match message {
                                    Correspondence::User { text, .. } => Some(text.clone()),
                                    Correspondence::Bot { .. } => None,
                                })
                            })
                            .unwrap_or_else(|| tr(language, "Temporary chat").to_string());
                        let title = format!("T · {}", ellipsize_chat_title(&title, 16));
                        entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(title).size(13).wrapping(Wrapping::None)
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(42.0))
                                    .girth(Length::Fixed(4.0)),
                            ])
                            .padding(4)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    let mut temporary_sessions = self
                        .temporary_chats
                        .iter()
                        .filter(|(_, session)| session.profile_id == self.active_profile_id)
                        .collect::<Vec<_>>();
                    temporary_sessions.sort_by_key(|(chat_id, _)| *chat_id);
                    for (chat_id, session) in temporary_sessions {
                        let title = session
                            .chat_history
                            .lock()
                            .ok()
                            .and_then(|chat| {
                                chat.messages.iter().find_map(|message| match message {
                                    Correspondence::User { text, .. } => Some(text.clone()),
                                    Correspondence::Bot { .. } => None,
                                })
                            })
                            .unwrap_or_else(|| tr(language, "Temporary chat").to_string());
                        let title = format!("T · {}", ellipsize_chat_title(&title, 16));
                        entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(title).size(13).wrapping(Wrapping::None)
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                mini_button("×", Message::DeleteTemporaryChat(chat_id.clone())),
                            ])
                            .padding(4)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    if has_temporary_chats {
                        entries.push(Space::new().height(Length::Fixed(8.0)).into());
                    }
                    entries.push(
                        widget::text(tr(language, "Saved chats"))
                            .size(13)
                            .color(text_muted())
                            .into(),
                    );
                    for saved in self
                        .saved_chats
                        .iter()
                        .filter(|chat| crate::chat_profile_id(chat) == self.active_profile_id)
                    {
                        let selected = saved.id == self.current_chat_id;
                        let title = ellipsize_chat_title(&saved.title, 20);
                        let working = self.active_prompts.contains_key(&saved.id);
                        let working_progress: Element<Message> = if working {
                            widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                .length(Length::Fixed(42.0))
                                .girth(Length::Fixed(4.0))
                                .into()
                        } else {
                            widget::column![].into()
                        };
                        entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(title).size(13).wrapping(Wrapping::None)
                                )
                                .on_press(Message::OpenChat(saved.id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                working_progress,
                                mini_button(
                                    tr(language, if saved.pinned { "Unpin" } else { "Pin" }),
                                    Message::ToggleChatPin(saved.id.clone()),
                                ),
                                mini_button("×", Message::DeleteChat(saved.id.clone())),
                            ])
                            .padding(4)
                            .width(Length::Fill)
                            .style(chat_entry_style(selected))
                            .into(),
                        );
                    }
                    container(widget::column![
                        widget::row![
                            container(
                                widget::image(self.brand_icon.clone())
                                    .width(Length::Fixed(25.0))
                                    .height(Length::Fixed(25.0))
                                    .content_fit(iced::ContentFit::Contain)
                                    .border_radius(8.0)
                            )
                            .padding(4)
                            .center_x(Length::Fill)
                            .center_y(Length::Fill)
                            .width(Length::Fixed(35.0))
                            .height(Length::Fixed(35.0))
                            .style(status_brand_style(status_color)),
                            Space::new().width(Length::Fixed(9.0)),
                            widget::column![
                                widget::text("LOCORYN").size(10).color(accent_2()),
                                widget::text(tr(language, "Chats"))
                                    .size(17)
                                    .color(text_main()),
                            ],
                            Space::new().width(Length::Fill),
                            mini_button("«", Message::ToggleChatMenu),
                        ],
                        Space::new().height(Length::Fixed(18.0)),
                        widget::scrollable(
                            widget::Column::with_children(entries).spacing(iced::Pixels(6.0))
                        ),
                        // Reserve room for the profile switcher overlaid in
                        // the bottom-left corner.
                        Space::new().height(Length::Fixed(46.0)),
                    ])
                    .padding(12)
                    .width(Length::Fixed(sidebar_width))
                    .height(Length::Fill)
                    .style(sidebar_style)
                    .into()
                } else {
                    let mut compact_entries: Vec<Element<Message>> = vec![
                        mini_button("☰", Message::ToggleChatMenu),
                        mini_button("＋", Message::NewChat),
                        mini_button(
                            if self.temporary_chat {
                                "◌ ✓"
                            } else {
                                "◌"
                            },
                            Message::ToggleTemporaryChat,
                        ),
                        Space::new().height(Length::Fixed(4.0)).into(),
                    ];
                    let mut temporary_jobs = self
                        .active_prompts
                        .iter()
                        .filter(|(_, job)| job.temporary && job.profile_id == self.active_profile_id)
                        .collect::<Vec<_>>();
                    temporary_jobs.sort_by_key(|(_, job)| job.started_at);
                    for (chat_id, _) in temporary_jobs {
                        compact_entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text("T · work").size(11).wrapping(Wrapping::None),
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .padding([6, 4])
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(14.0))
                                    .girth(Length::Fixed(3.0)),
                            ])
                            .padding(2)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    let mut temporary_sessions = self
                        .temporary_chats
                        .iter()
                        .filter(|(_, session)| session.profile_id == self.active_profile_id)
                        .map(|(chat_id, _)| chat_id)
                        .collect::<Vec<_>>();
                    temporary_sessions.sort();
                    for chat_id in temporary_sessions {
                        compact_entries.push(
                            container(
                                widget::button(
                                    widget::text("T · done").size(11).wrapping(Wrapping::None),
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .padding([6, 4])
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                            )
                            .padding(2)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    for saved in self
                        .saved_chats
                        .iter()
                        .filter(|chat| crate::chat_profile_id(chat) == self.active_profile_id)
                    {
                        let short_title = ellipsize_chat_title(&saved.title, 8);
                        let working = self.active_prompts.contains_key(&saved.id);
                        let working_progress: Element<Message> = if working {
                            widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                .length(Length::Fixed(14.0))
                                .girth(Length::Fixed(3.0))
                                .into()
                        } else {
                            widget::column![].into()
                        };
                        compact_entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(short_title).size(11).wrapping(Wrapping::None),
                                )
                                .on_press(Message::OpenChat(saved.id.clone()))
                                .padding([6, 4])
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                working_progress,
                            ])
                            .padding(2)
                            .width(Length::Fill)
                            .style(chat_entry_style(saved.id == self.current_chat_id))
                            .into(),
                        );
                    }
                    container(widget::column![
                        widget::scrollable(
                            widget::Column::with_children(compact_entries)
                                .spacing(iced::Pixels(5.0)),
                        ),
                        // Reserve room for the profile switcher overlaid in
                        // the bottom-left corner.
                        Space::new().height(Length::Fixed(46.0)),
                    ])
                    .padding(8)
                    .width(Length::Fixed(sidebar_width))
                    .height(Length::Fill)
                    .style(sidebar_style)
                    .into()
                };

                let active_profile_name = self.active_profile_name();
                let profile_chip_label: Element<Message> = if show_sidebar_details {
                    widget::row![
                        widget::text("👤").size(13),
                        widget::text(ellipsize_chat_title(&active_profile_name, 18))
                            .size(14)
                            .wrapping(Wrapping::None),
                        widget::text(if self.profile_menu_open { "▾" } else { "▴" })
                            .size(11)
                            .color(text_muted()),
                    ]
                    .spacing(7)
                    .into()
                } else {
                    widget::text("👤").size(14).into()
                };
                let profile_chip: Element<Message> = widget::button(profile_chip_label)
                    .on_press(Message::ToggleProfileMenu)
                    .padding(if show_sidebar_details { [8, 13] } else { [8, 9] })
                    .style(profile_chip_style(self.profile_menu_open))
                    .into();
                let profile_switcher: Element<Message> = if self.profile_menu_open {
                    let mut rows: Vec<Element<Message>> = Vec::new();
                    for profile in &self.profiles {
                        let is_active = profile.id == self.active_profile_id;
                        let is_editing = self
                            .editing_profile_id
                            .as_deref()
                            .is_some_and(|editing| editing == profile.id);
                        let active_marker: Element<Message> = if is_active {
                            widget::text("✓").size(13).color(accent_2()).into()
                        } else {
                            widget::column![].into()
                        };
                        rows.push(
                            widget::row![
                                widget::button(
                                    widget::text(ellipsize_chat_title(&profile.name, 24))
                                        .size(14)
                                        .wrapping(Wrapping::None)
                                        .color(if is_active {
                                            accent_2()
                                        } else {
                                            text_main()
                                        }),
                                )
                                .on_press(Message::SelectProfile(profile.id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                active_marker,
                                mini_button(
                                    if is_editing { "▾" } else { "✎" },
                                    Message::StartEditProfile(profile.id.clone())
                                ),
                                mini_button("×", Message::DeleteProfile(profile.id.clone())),
                            ]
                            .spacing(5)
                            .into(),
                        );
                    }
                    let rows_height = (rows.len().min(5) * 40) as f32;
                    let edit_panel: Element<Message> =
                        if self.editing_profile_id.is_some()
                            && self.profiles.iter().any(|profile| {
                                Some(profile.id.as_str())
                                    == self.editing_profile_id.as_deref()
                            })
                        {
                            container(
                                widget::column![
                                    widget::text(tr(language, "EDIT PROFILE"))
                                        .size(10)
                                        .color(accent_2()),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "Profile name (only you see this)"),
                                        &self.profile_edit_name,
                                    )
                                    .on_input(Message::ProfileEditNameChanged)
                                    .padding(9)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "User name (shared with the model)"),
                                        &self.profile_edit_user_name,
                                    )
                                    .on_input(Message::ProfileEditUserNameChanged)
                                    .padding(9)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "Extra instructions for the model"),
                                        &self.profile_edit_instructions,
                                    )
                                    .on_input(Message::ProfileEditInstructionsChanged)
                                    .on_submit(Message::ConfirmProfileEdits)
                                    .padding(9)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                    widget::row![
                                        Space::new().width(Length::Fill),
                                        mini_button(
                                            tr(language, "Cancel"),
                                            Message::CancelEditProfile
                                        ),
                                        mini_button(
                                            tr(language, "Save"),
                                            Message::ConfirmProfileEdits
                                        ),
                                    ]
                                    .spacing(5),
                                ]
                                .spacing(7),
                            )
                            .padding(10)
                            .width(Length::Fill)
                            .style(flat_card_style)
                            .into()
                        } else {
                            widget::column![].into()
                        };
                    let popup: Element<Message> = container(
                        widget::column![
                            widget::text(tr(language, "PROFILES"))
                                .size(11)
                                .color(accent_2()),
                            widget::scrollable(
                                widget::Column::with_children(rows).spacing(iced::Pixels(6.0))
                            )
                            .height(Length::Fixed(rows_height)),
                            edit_panel,
                            widget::row![
                                iced::widget::TextInput::<Message>::new(
                                    tr(language, "New profile name"),
                                    &self.profile_name_input,
                                )
                                .on_input(Message::ProfileNameInputChanged)
                                .on_submit(Message::CreateProfile)
                                .padding(9)
                                .width(Length::Fill)
                                .style(text_input_style),
                                mini_button("＋", Message::CreateProfile),
                            ]
                            .spacing(5),
                        ]
                        .spacing(9),
                    )
                    .padding(12)
                    .width(Length::Fixed(290.0))
                    .style(profile_popup_style)
                    .into();
                    widget::column![
                        popup,
                        Space::new().height(Length::Fixed(6.0)),
                        profile_chip,
                    ]
                    .align_x(iced::Alignment::Start)
                    .into()
                } else {
                    profile_chip
                };
                // The switcher floats in the bottom-left corner, over the
                // chat list, so it stays reachable on every page state.
                let chat_sidebar: Element<Message> = widget::stack![
                    chat_sidebar,
                    container(profile_switcher)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::Alignment::Start)
                        .align_y(iced::Alignment::End)
                        .padding(10),
                ]
                .into();

                let copy_response_action: Element<Message> = if chat_is_empty {
                    widget::column![].into()
                } else {
                    mini_button(tr(language, "Copy response"), Message::CopyLatestResponse)
                };
                let composer_active =
                    !self.prompt.prompt.trim().is_empty() || !self.pending_images.is_empty();
                let prompt_input: Element<Message> = container(prompt)
                    .padding(3)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(input_shell_style(composer_active, pulse))
                    .into();
                let composer_input: Element<Message> = if self.pending_images.is_empty() {
                    prompt_input
                } else {
                    widget::row![
                        composer_image_previews(&self.pending_images),
                        Space::new().width(Length::Fixed(8.0)),
                        prompt_input,
                    ]
                    .align_y(iced::Alignment::Center)
                    .height(Length::Fill)
                    .into()
                };

                let content = widget::column![
                    Space::new().height(Length::Fixed((1.0 - eased(self.page_reveal)) * 4.0)),
                    container(widget::column![
                        widget::row![
                            widget::column![
                                widget::text(tr(language, "LOCAL AI WORKSPACE"))
                                    .size(10)
                                    .color(accent_2()),
                                Space::new().height(Length::Fixed(3.0)),
                                widget::text(current_chat_title).size(20).color(text_main()),
                            ],
                            Space::new().width(Length::Fill),
                            toolbar_button("▣", tr(language, "Images"), Message::ToggleImages),
                            Space::new().width(Length::Fixed(6.0)),
                            toolbar_button("⚙", tr(language, "Settings"), Message::ToggleSettings),
                        ],
                        Space::new().height(Length::Fixed(16.0)),
                        widget::row![
                            widget::column![
                                widget::text(tr(language, "MODEL"))
                                    .size(10)
                                    .color(text_faint()),
                                Space::new().height(Length::Fixed(5.0)),
                                container(model_selector).width(Length::Fill),
                            ]
                            .width(Length::FillPortion(5)),
                            Space::new().width(Length::Fixed(10.0)),
                            widget::column![
                                widget::text(tr(language, "SYSTEM PROMPT"))
                                    .size(10)
                                    .color(text_faint()),
                                Space::new().height(Length::Fixed(5.0)),
                                widget::pick_list(
                                    self.system_prompt
                                        .system_prompts_as_vec
                                        .lock()
                                        .unwrap()
                                        .clone(),
                                    self.system_prompt.system_prompt.clone(),
                                    Message::SystemPromptChange,
                                )
                                .placeholder(tr(language, "System prompt"))
                                .padding([12, 14])
                                .text_size(14)
                                .style(pick_list_style)
                                .menu_style(pick_list_menu_style)
                                .width(Length::Fill),
                            ]
                            .width(Length::FillPortion(3)),
                            Space::new().width(Length::Fixed(10.0)),
                            widget::column![
                                widget::text(tr(language, "REASONING"))
                                    .size(10)
                                    .color(text_faint()),
                                Space::new().height(Length::Fixed(5.0)),
                                thinking_selector,
                            ]
                            .width(Length::Fixed(150.0)),
                        ],
                    ])
                    .padding(16)
                    .width(Length::Fill)
                    .style(top_bar_style),
                    Space::new().height(Length::Fixed(10.0)),
                    container(
                        widget::scrollable(
                            widget::column![
                                widget::Column::with_children(chat_widgets)
                                    .spacing(iced::Pixels(3.0)),
                                live_response,
                                Space::new().height(Length::Fixed(18.0)),
                            ]
                            .spacing(iced::Pixels(6.0))
                        )
                        .height(Length::Fill)
                        .anchor_bottom()
                    )
                    .padding([20, 18])
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(conversation_style),
                    composer_resize_handle(),
                    container(
                        widget::column![
                            composer_input,
                            Space::new().height(Length::Fixed(9.0)),
                            widget::row![
                                mini_button(tr(language, "＋ Attach"), Message::PickImage),
                                Space::new().width(Length::Fixed(5.0)),
                                mini_button(tr(language, "Paste"), Message::PasteImage),
                                Space::new().width(Length::Fixed(5.0)),
                                web_toggle,
                                Space::new().width(Length::Fill),
                                widget::text(tr(language, "Enter to send"))
                                    .size(11)
                                    .color(text_faint()),
                                Space::new().width(Length::Fixed(8.0)),
                                copy_response_action,
                                Space::new().width(Length::Fixed(6.0)),
                                if is_processing {
                                    danger_button(tr(language, "■ Stop"), Message::StopResponse)
                                } else {
                                    send_button(
                                        tr(language, "Send"),
                                        (!self.prompt.prompt.trim().is_empty())
                                            .then(|| Message::Prompt(self.prompt.prompt.clone())),
                                    )
                                },
                            ],
                            offline_hint,
                            missing_bots_hint,
                            widget::row![
                                widget::text(visible_debug.message)
                                    .size(13)
                                    .color(debug_color),
                            ],
                        ]
                        .height(Length::Fill)
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .height(Length::Fixed(self.ui_layout.composer_height))
                    .style(composer_style(composer_active, pulse)),
                ]
                .spacing(iced::Pixels(0.0));

                let sidebar_handle: Element<Message> =
                    if self.chat_menu_open && self.sidebar_animation >= 0.998 {
                        sidebar_resize_handle()
                    } else {
                        Space::new().width(Length::Fixed(10.0)).into()
                    };
                container(widget::row![chat_sidebar, sidebar_handle, content])
                    .padding(10)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::Images => {
                let bots_list = self.app_state.bots_list.lock().unwrap().clone();
                let selected_model = self.user_information.model.clone();
                let active_prompt = self.current_active_prompt();
                let vision_is_live = active_prompt.is_some_and(|job| job.had_image);
                let completed_vision = self.vision_responses.get(&self.current_chat_id);
                let visible_debug = self.current_debug_message().clone();
                let pulse = 1.0 - (self.ui_motion * 2.0 - 1.0).abs();
                let model_selector =
                    widget::pick_list(bots_list, selected_model, Message::ModelChange)
                        .padding([12, 14])
                        .text_size(14)
                        .style(pick_list_style)
                        .menu_style(pick_list_menu_style)
                        .width(Length::Fill);

                let attachment: Element<Message> = if !self.pending_images.is_empty() {
                    image_previews(&self.pending_images, true, language)
                } else {
                    container(widget::column![
                        widget::text(tr(language, "Add an image for vision"))
                            .size(17)
                            .color(text_main()),
                        Space::new().height(Length::Fixed(5.0)),
                        widget::text(tr(
                            language,
                            "Paste from the clipboard or choose a local image."
                        ))
                        .size(12)
                        .color(text_muted()),
                        Space::new().height(Length::Fixed(14.0)),
                        widget::row![
                            secondary_button(tr(language, "Choose image"), Message::PickImage),
                            Space::new().width(Length::Fixed(8.0)),
                            secondary_button(tr(language, "Paste image"), Message::PasteImage),
                        ],
                    ])
                    .padding(22)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                };

                let capability_status: Element<Message> = match self
                    .user_information
                    .vision_supported
                {
                    Some(true) => {
                        container(widget::text(tr(language, "This model can inspect images.")))
                            .padding(9)
                            .style(chip_style(success()))
                            .into()
                    }
                    Some(false) => container(widget::text(tr(
                        language,
                        "This model does not support image input.",
                    )))
                    .padding(9)
                    .style(chip_style(danger()))
                    .into(),
                    None => container(widget::text(tr(language, "Checking image capabilities…")))
                        .padding(9)
                        .style(chip_style(brighten(warning(), pulse * 0.025)))
                        .into(),
                };

                let vision_prompt_to_send = self.prompt.prompt.clone();
                let vision_prompt = widget::text_editor(&self.prompt.editor)
                    .placeholder(tr(
                        language,
                        "Describe an image, or ask a question about the attached image…",
                    ))
                    .padding(14)
                    .size(16)
                    .min_height(72)
                    .max_height(180)
                    .on_action(Message::EditPrompt)
                    .key_binding(move |key_press| {
                        if matches!(
                            key_press.key.as_ref(),
                            iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                        ) && !key_press.modifiers.shift()
                        {
                            Some(widget::text_editor::Binding::Custom(Message::Prompt(
                                vision_prompt_to_send.clone(),
                            )))
                        } else {
                            widget::text_editor::Binding::from_key_press(key_press)
                        }
                    })
                    .style(text_editor_style);

                let vision_action: Element<Message> = if vision_is_live {
                    primary_button(tr(language, "Stop"), Message::StopResponse)
                } else if self.user_information.vision_supported != Some(false)
                    && !self.pending_images.is_empty()
                {
                    primary_button(
                        tr(language, "Ask about image"),
                        Message::Prompt(self.prompt.prompt.clone()),
                    )
                } else {
                    widget::column![].into()
                };

                let vision_response: Element<Message> = if vision_is_live
                    || completed_vision.is_some()
                {
                    let markdown = active_prompt
                        .filter(|job| job.had_image)
                        .map(|job| job.parsed_markdown.as_slice())
                        .or_else(|| completed_vision.map(|response| response.markdown.as_slice()))
                        .unwrap_or_default();
                    container(widget::column![
                        widget::row![
                            widget::text(tr(
                                language,
                                if vision_is_live {
                                    "Vision model is responding…"
                                } else {
                                    "Vision response"
                                }
                            ))
                            .size(12)
                            .color(accent_2()),
                            Space::new().width(Length::Fill),
                            if vision_is_live {
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(96.0))
                                    .girth(Length::Fixed(5.0))
                            } else {
                                widget::progress_bar(0.0..=1.0, 0.0)
                                    .length(Length::Fixed(0.0))
                                    .girth(Length::Fixed(0.0))
                            },
                        ],
                        Space::new().height(Length::Fixed(8.0)),
                        markdown_with_code_copy(
                            markdown,
                            self.user_information.text_size,
                            self.last_copied_text.as_ref(),
                            language,
                            self.code_checking_enabled,
                            &self.markdown_images,
                            self.ui_motion,
                        ),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(bot_bubble_style(eased(self.page_reveal)))
                    .into()
                } else {
                    widget::column![].into()
                };

                let generated_cards: Vec<Element<Message>> = self
                    .generated_images
                    .iter()
                    .rev()
                    .map(|path| {
                        container(widget::column![
                            widget::image(iced::widget::image::Handle::from_path(path))
                                .height(Length::Fixed(280.0))
                                .width(Length::Fill)
                                .content_fit(iced::ContentFit::Contain)
                                .border_radius(12.0)
                                .opacity(eased(self.page_reveal))
                                .scale(0.99 + eased(self.page_reveal) * 0.01),
                            Space::new().height(Length::Fixed(8.0)),
                            widget::row![
                                widget::text(path.clone())
                                    .size(11)
                                    .color(text_muted())
                                    .wrapping(Wrapping::WordOrGlyph)
                                    .width(Length::Fill),
                                mini_button(
                                    tr(language, "Copy image"),
                                    Message::CopyImage(path.clone())
                                ),
                            ],
                        ])
                        .padding(12)
                        .width(Length::Fill)
                        .style(flat_card_style)
                        .into()
                    })
                    .collect();

                let generation_progress: Element<Message> = if self.is_generating_image {
                    widget::progress_bar(0.0..=1.0, 0.08 + pulse * 0.84)
                        .girth(Length::Fixed(4.0))
                        .into()
                } else {
                    Space::new().height(Length::Fixed(0.0)).into()
                };

                let generation_panel: Element<Message> = if self
                    .user_information
                    .image_generation_supported
                    == Some(true)
                {
                    let generation_prompt = widget::text_editor(&self.prompt.editor)
                        .placeholder(tr(language, "Describe the image you want to generate…"))
                        .padding(14)
                        .size(16)
                        .min_height(72)
                        .max_height(180)
                        .on_action(Message::EditPrompt)
                        .key_binding(|key_press| {
                            if matches!(
                                key_press.key.as_ref(),
                                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                            ) && !key_press.modifiers.shift()
                            {
                                Some(widget::text_editor::Binding::Custom(Message::GenerateImage))
                            } else {
                                widget::text_editor::Binding::from_key_press(key_press)
                            }
                        })
                        .style(text_editor_style);
                    container(widget::column![
                            setting_label(
                                tr(language, "Experimental image generation"),
                                tr(language, "Ollama reports that this model can generate images. Output is requested through /v1/images/generations at 1024 × 1024.")
                            ),
                            Space::new().height(Length::Fixed(12.0)),
                            generation_prompt,
                            Space::new().height(Length::Fixed(10.0)),
                            primary_button(
                                tr(
                                    language,
                                    if self.is_generating_image {
                                        "Generating…"
                                    } else {
                                        "Generate image"
                                    }
                                ),
                                Message::GenerateImage
                            ),
                            generation_progress,
                        ])
                        .padding(18)
                        .width(Length::Fill)
                        .style(conversation_style)
                        .into()
                } else {
                    widget::column![].into()
                };

                let generated_gallery: Element<Message> = if generated_cards.is_empty() {
                    widget::column![].into()
                } else {
                    container(widget::column![
                        widget::text(tr(language, "Generated images"))
                            .size(18)
                            .color(text_main()),
                        Space::new().height(Length::Fixed(10.0)),
                        widget::Column::with_children(generated_cards).spacing(iced::Pixels(12.0)),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(conversation_style)
                    .into()
                };

                let content = widget::column![
                    Space::new().height(Length::Fixed(
                        (1.0 - eased(self.page_reveal)) * 4.0
                    )),
                    container(widget::row![
                        section_title(
                            tr(language, "Images"),
                            tr(language, "Analyze images with a vision model. Experimental image generation appears only for models that report support.")
                        ),
                        Space::new().width(Length::Fill),
                        secondary_button(tr(language, "Back to chat"), Message::ToggleImages),
                    ]).padding(18).width(Length::Fill).style(top_bar_style),
                    Space::new().height(Length::Fixed(14.0)),
                    container(widget::column![
                        setting_label(
                            tr(language, "Vision analysis"),
                            tr(language, "Attach an image and ask a vision-capable model to describe, classify, read, or reason about it.")
                        ),
                        Space::new().height(Length::Fixed(12.0)),
                        widget::text(tr(language, "Model"))
                            .size(12)
                            .color(text_faint()),
                        Space::new().height(Length::Fixed(5.0)),
                        model_selector,
                        Space::new().height(Length::Fixed(10.0)),
                        capability_status,
                        Space::new().height(Length::Fixed(14.0)),
                        attachment,
                        Space::new().height(Length::Fixed(12.0)),
                        vision_prompt,
                        Space::new().height(Length::Fixed(10.0)),
                        vision_action,
                    ]).padding(18).width(Length::Fill).style(conversation_style),
                    Space::new().height(Length::Fixed(14.0)),
                    vision_response,
                    Space::new().height(Length::Fixed(14.0)),
                    generation_panel,
                    Space::new().height(Length::Fixed(14.0)),
                    generated_gallery,
                    widget::text(visible_debug.message)
                        .size(13)
                        .color(if visible_debug.is_error { danger() } else { success() }),
                ];

                container(widget::scrollable(content))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::Settings => {
                let user_information = self.user_information.clone();
                let web_api_key = self
                    .web_search_settings
                    .selected_api_key()
                    .unwrap_or_default()
                    .to_string();
                let bots_list = self.app_state.bots_list.lock().unwrap().clone();
                let prompts_list = self
                    .system_prompt
                    .system_prompts_as_vec
                    .lock()
                    .unwrap()
                    .clone();

                let debug_color = if self.debug_message.clone().is_error {
                    danger()
                } else {
                    success()
                };
                let (update_status, update_action): (String, Element<Message>) =
                    match &self.app_update_state {
                        AppUpdateState::Idle => (
                            format!("Current version: v{}", crate::APP_VERSION),
                            secondary_button(tr(language, "Check now"), Message::CheckForUpdates),
                        ),
                        AppUpdateState::Checking => (
                            format!("Current version: v{}", crate::APP_VERSION),
                            container(
                                widget::text(tr(language, "Checking…"))
                                    .size(12)
                                    .color(accent_2()),
                            )
                            .padding([8, 10])
                            .style(chip_style(accent_2()))
                            .into(),
                        ),
                        AppUpdateState::UpToDate { latest_version } => (
                            format!(
                                "Up to date · v{} (latest: v{latest_version})",
                                crate::APP_VERSION
                            ),
                            secondary_button(tr(language, "Check now"), Message::CheckForUpdates),
                        ),
                        AppUpdateState::Available {
                            latest_version,
                            release_url,
                        } => (
                            format!(
                                "Update available · v{} → v{latest_version}",
                                crate::APP_VERSION
                            ),
                            primary_button(
                                tr(language, "Download update"),
                                Message::OpenAppUpdate(release_url.clone()),
                            ),
                        ),
                        AppUpdateState::Failed(error) => (
                            format!("Update check failed: {error}"),
                            secondary_button(tr(language, "Check now"), Message::CheckForUpdates),
                        ),
                    };

                let content = widget::column![
                    Space::new().height(Length::Fixed(
                        (1.0 - eased(self.page_reveal)) * 4.0
                    )),
                    container(
                        widget::row![
                            section_title(
                                tr(language, "Settings"),
                                tr(language, "Tune model behaviour, prompt selection, and chat preferences.")
                            ),
                            Space::new().width(Length::Fill),
                            secondary_button(tr(language, "Go back"), Message::ToggleSettings),
                        ]
                    )
                    .padding(18)
                    .width(Length::Fill)
                    .style(top_bar_style),

                    Space::new().height(Length::Fixed(14.0)),

                    container(
                        widget::row![
                            widget::column![
                                Space::new().height(Length::Fixed(
                                    (1.0 - eased(self.page_reveal * 1.12)) * 14.0
                                )),
                                widget::column![
                            settings_group_title(tr(language, "PERSONALIZATION")),
                            Space::new().height(Length::Fixed(8.0)),
                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Interface language"),
                                        tr(language, "Spanish is experimental and machine-generated. It will be replaced with a human translation in a future update.")
                                    ),
                                    widget::pick_list(
                                        [Language::English, Language::Spanish],
                                        Some(language),
                                        Message::LanguageChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Dynamic system prompt"),
                                        tr(language, "Add current local information and your own instructions to every request. Each profile can add its own user name and instructions; profile names stay private. Switch profiles from the bottom-left corner.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::checkbox(self.dynamic_prompt_settings.include_date)
                                            .label(tr(language, "Include date"))
                                            .on_toggle(|_| Message::ToggleDynamicDate),
                                        Space::new().width(Length::Fill),
                                        widget::checkbox(self.dynamic_prompt_settings.include_time)
                                            .label(tr(language, "Include time"))
                                            .on_toggle(|_| Message::ToggleDynamicTime),
                                    ],
                                    Space::new().height(Length::Fixed(10.0)),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "Custom instructions appended to the system prompt"),
                                        &self.dynamic_prompt_settings.custom_instructions,
                                    )
                                    .on_input(Message::DynamicCustomInstructionsChanged)
                                    .padding(11)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "MODEL & RESPONSES")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Maximum response"),
                                        tr(language, "Caps generated output in tokens, including hidden reasoning. The default is 32,768; direct entry supports up to 1,048,576.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            9.0..=20.0,
                                            self.user_information.max_response_tokens.ilog2() as f32,
                                            Message::UpdateMaxResponseTokens,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        iced::widget::TextInput::<Message>::new(
                                            "tokens",
                                            &self.max_response_tokens_input,
                                        )
                                        .on_input(Message::EditMaxResponseTokens)
                                        .on_submit(Message::ApplyMaxResponseTokens)
                                        .padding(
                                            9.0 + self.settings_feedback(
                                                SettingsFeedbackTarget::MaxResponse
                                            ) * 0.8
                                        )
                                        .width(Length::Fixed(130.0))
                                        .style(feedback_text_input_style(
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::MaxResponse
                                            )
                                        )),
                                        feedback_apply_button(
                                            "Apply",
                                            Message::ApplyMaxResponseTokens,
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ApplyMaxResponse
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Context window"),
                                        tr(language, "Controls how much conversation and generated output the model can hold. Larger values use substantially more memory.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            12.0..=22.0,
                                            self.user_information.context_tokens.ilog2() as f32,
                                            Message::UpdateContextTokens,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        iced::widget::TextInput::<Message>::new(
                                            "tokens",
                                            &self.context_tokens_input,
                                        )
                                        .on_input(Message::EditContextTokens)
                                        .on_submit(Message::ApplyContextTokens)
                                        .padding(
                                            9.0 + self.settings_feedback(
                                                SettingsFeedbackTarget::ContextWindow
                                            ) * 0.8
                                        )
                                        .width(Length::Fixed(130.0))
                                        .style(feedback_text_input_style(
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ContextWindow
                                            )
                                        )),
                                        feedback_apply_button(
                                            "Apply",
                                            Message::ApplyContextTokens,
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ApplyContextWindow
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Model"),
                                        tr(language, "Choose the Ollama model used for new responses.")
                                    ),
                                    widget::pick_list(
                                        bots_list,
                                        self.user_information.model.clone(),
                                        Message::ModelChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            if self.user_information.thinking_supported == Some(true) {
                                container(
                                    widget::column![
                                        setting_label(
                                            tr(language, "Thinking effort"),
                                            tr(language, "Choose how much reasoning the model should use.")
                                        ),
                                        Space::new().height(Length::Fixed(10.0)),
                                        thinking_control(
                                            self.user_information.thinking_level,
                                            &self.user_information.thinking_levels,
                                            language,
                                        ),
                                    ]
                                )
                                .padding(16)
                                .width(Length::Fill)
                                .style(flat_card_style)
                            } else {
                                container(
                                    setting_label(
                                        tr(language, "Reasoning"),
                                        if self.user_information.thinking_supported == Some(false) {
                                            tr(language, "This model does not offer adjustable reasoning.")
                                        } else {
                                            tr(language, "Select a model and wait while reasoning support is checked.")
                                        }
                                    )
                                )
                                .padding(16)
                                .width(Length::Fill)
                                .style(flat_card_style)
                            },

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Temperature"),
                                        tr(language, "Higher values make output more random.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            0.0..=10.0,
                                            self.user_information.temperature,
                                            Message::UpdateTemperature,
                                        ),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            format!("{:.1}", self.user_information.temperature),
                                            accent(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::Temperature
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "System prompt"),
                                        tr(language, "Choose the personality or instruction profile.")
                                    ),
                                    widget::pick_list(
                                        prompts_list,
                                        self.system_prompt.system_prompt.clone(),
                                        Message::SystemPromptChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                                ]
                            ]
                            .width(Length::Fill),

                            Space::new().width(Length::Fixed(14.0)),

                            widget::column![
                                Space::new().height(Length::Fixed(
                                    (1.0 - eased(self.page_reveal * 1.18 - 0.08)) * 22.0
                                )),
                                widget::column![
                            settings_group_title(tr(language, "APPEARANCE")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Text size"),
                                        tr(language, "Adjust chat and response readability.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            1.0..=40.0,
                                            self.user_information.text_size,
                                            Message::UpdateTextSize,
                                        ),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            format!("{:.0}px", self.user_information.text_size),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::TextSize
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::row![
                                    setting_label(
                                        tr(language, "Dark mode"),
                                        tr(language, "Switch between the dark and light interface themes.")
                                    ),
                                    widget::checkbox(self.app_state.dark_mode)
                                        .label(tr(language, "Enabled"))
                                        .on_toggle(|_| Message::ToggleDarkMode),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "WEB SEARCH & TOOLS")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    widget::row![
                                        setting_label(
                                            tr(language, "Enable Web Search"),
                                            tr(language, "Web search may send search queries and webpage URLs to the selected external provider.")
                                        ),
                                        widget::checkbox(self.web_search_settings.enabled)
                                            .label(tr(language, "Enabled"))
                                            .on_toggle(|_| Message::ToggleWebSearch),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Search provider"),
                                        "The provider is contacted only while web search is enabled. Tavily and Exa support is experimental."
                                    ),
                                    widget::pick_list(
                                        crate::web_search::WebSearchProviderKind::ALL,
                                        Some(self.web_search_settings.provider),
                                        Message::WebSearchProviderChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "API key"),
                                        self.web_search_settings.provider.api_key_help()
                                    ),
                                    iced::widget::TextInput::<Message>::new(
                                        self.web_search_settings.provider.api_key_placeholder(),
                                        &web_api_key,
                                    )
                                    .secure(true)
                                    .padding(12)
                                    .on_input(Message::WebSearchApiKeyChange)
                                    .style(text_input_style),
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Search result limit"),
                                        "Limits results returned by each model-requested search."
                                    ),
                                    widget::row![
                                        widget::slider(
                                            1.0..=crate::web_search::MAX_RESULT_LIMIT as f32,
                                            self.web_search_settings.result_limit as f32,
                                            Message::WebSearchResultLimitChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.result_limit.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::SearchResultLimit
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    widget::row![
                                        setting_label(
                                            tr(language, "Deep follow-up research"),
                                            tr(language, "Use the configurable multi-query and cross-source research budget below.")
                                        ),
                                        widget::checkbox(
                                            self.web_search_settings.allow_multiple_searches
                                        )
                                        .label(tr(language, "Enabled"))
                                        .on_toggle(|_| Message::ToggleMultipleWebSearches),
                                    ],
                                    Space::new().height(Length::Fixed(16.0)),
                                    settings_disclosure_button(
                                        tr(language, "Deep research controls"),
                                        self.deep_research_controls_open,
                                        Message::ToggleDeepResearchControls,
                                    ),
                                    if self.deep_research_controls_open {
                                    widget::column![
                                    Space::new().height(Length::Fixed(10.0 +
                                        (1.0 - eased(self.page_reveal)) * 4.0
                                    )),
                                    setting_label(
                                        tr(language, "Maximum searches"),
                                        tr(language, "Hard cap on distinct search queries in one deep-research response.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            1.0..=crate::web_search::MAX_CONFIGURABLE_SEARCHES as f32,
                                            self.web_search_settings.maximum_searches as f32,
                                            Message::WebSearchMaximumSearchesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.maximum_searches.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::MaximumSearches
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Required successful searches"),
                                        tr(language, "The model is reminded to keep researching until this many distinct searches succeed.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            1.0..=self.web_search_settings.maximum_searches as f32,
                                            self.web_search_settings.minimum_successful_searches as f32,
                                            Message::WebSearchMinimumSuccessfulSearchesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.minimum_successful_searches.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::RequiredSearches
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Maximum page reads"),
                                        tr(language, "Hard cap on full webpage fetches in one deep-research response. Set to 0 to disable page reads.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            0.0..=crate::web_search::MAX_CONFIGURABLE_PAGES as f32,
                                            self.web_search_settings.maximum_page_fetches as f32,
                                            Message::WebSearchMaximumPageFetchesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.maximum_page_fetches.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::MaximumPageReads
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Required independent pages"),
                                        tr(language, "Minimum number of different domains the model should inspect. Set to 0 to disable this checkpoint.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            0.0..=self.web_search_settings.maximum_page_fetches as f32,
                                            self.web_search_settings.minimum_independent_pages as f32,
                                            Message::WebSearchMinimumIndependentPagesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.minimum_independent_pages.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::RequiredPageReads
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Maximum tool rounds"),
                                        tr(language, "Caps model/tool hand-offs. Reaching this limit now forces a final answer instead of failing the prompt.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            2.0..=crate::web_search::MAX_CONFIGURABLE_TOOL_ITERATIONS as f32,
                                            self.web_search_settings.tool_iteration_limit as f32,
                                            Message::WebSearchToolIterationLimitChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.tool_iteration_limit.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ToolRounds
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Web request timeout"),
                                        tr(language, "Timeout for each external search or webpage request.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            3.0..=60.0,
                                            self.web_search_settings.request_timeout_seconds as f32,
                                            Message::WebSearchRequestTimeoutChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            format!(
                                                "{}s",
                                                self.web_search_settings.request_timeout_seconds
                                            ),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::RequestTimeout
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Custom research instructions"),
                                        tr(language, "Optional guidance added only to deep-research requests.")
                                    ),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "e.g. Prioritize official sources and compare opposing views"),
                                        &self.web_search_settings.custom_research_instructions,
                                    )
                                    .on_input(Message::WebSearchCustomInstructionsChange)
                                    .padding(11)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                    ]
                                    } else {
                                        widget::column![]
                                    },
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "DATA & MAINTENANCE")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::row![
                                    widget::column![
                                        setting_label(
                                            tr(language, "Application updates"),
                                            tr(language, "Current version and latest stable release from GitHub.")
                                        ),
                                        widget::text(update_status)
                                            .size(12)
                                            .color(text_muted())
                                            .wrapping(Wrapping::WordOrGlyph),
                                    ]
                                    .width(Length::Fill),
                                    update_action,
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::row![
                                    widget::column![
                                        setting_label(
                                            tr(language, "Chat storage"),
                                            tr(language, "Saved chats use this folder. The full path is shown so you can always locate them.")
                                        ),
                                        widget::text(self.chat_storage_dir.display().to_string())
                                            .size(12)
                                            .color(text_muted())
                                            .wrapping(Wrapping::WordOrGlyph),
                                    ]
                                    .width(Length::Fill),
                                    secondary_button(
                                        tr(language, "Choose folder"),
                                        Message::ChooseChatFolder
                                    ),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::row![
                                    setting_label(
                                        tr(language, "Model conversation context"),
                                        tr(language, "Include earlier messages from this chat in the next model request. Saved chats are managed in the left menu.")
                                    ),
                                    widget::checkbox(
                                        user_information.current_chat_history_enabled
                                    )
                                    .label(tr(language, "Enabled"))
                                    .on_toggle(|_| Message::ToggleChatHistory),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(14.0)),

                            container(
                                widget::column![
                                    widget::column![
                                        widget::text(tr(language, "Maintenance"))
                                            .size(16)
                                            .color(text_main()),
                                        Space::new().height(Length::Fixed(4.0)),
                                        widget::text(tr(language, "Clear local conversation data or open deeper configuration options."))
                                            .size(12)
                                            .color(text_muted()),
                                    ]
                                    .width(Length::Fill),

                                    Space::new().height(Length::Fixed(12.0)),

                                    widget::row![
                                        danger_button(
                                            tr(language, "Clear current context"),
                                            Message::WipeChatHistory
                                        ),

                                        Space::new().width(Length::Fill),

                                        secondary_button(
                                            tr(language, "Advanced settings"),
                                            Message::ToggleAdvancedSettings
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(danger_zone_style),

                            Space::new().height(Length::Fixed(12.0)),

                            widget::text(self.debug_message.clone().message)
                                .size(13)
                                .color(debug_color),
                                ]
                            ]
                            .width(Length::Fill),
                        ]
                    )
                    .padding(18)
                    .width(Length::Fill)
                    .style(conversation_style),
                ];

                container(widget::scrollable(content).height(Length::Fill))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::AdvancedSettings => {
                let user_information = self.user_information.clone();
                let ip = self.user_information.ip_address.clone();

                let prompts_list = self
                    .system_prompt
                    .system_prompts_as_vec
                    .lock()
                    .unwrap()
                    .clone();

                let model_install = iced::widget::TextInput::<Message>::new(
                    tr(language, "Model name, e.g. llama3.2:3b"),
                    &self.installing_model,
                )
                .padding(12)
                .size(15)
                .width(Length::Fill)
                .on_submit(Message::InstallModel(self.installing_model.clone()))
                .on_input(Message::UpdateInstall)
                .style(text_input_style);

                let change_ip = iced::widget::TextInput::<Message>::new(ip.ip.as_str(), &ip.ip)
                    .padding(12)
                    .size(15)
                    .width(Length::FillPortion(3))
                    .on_submit(Message::ChangeIp(ip.ip.clone()))
                    .on_input(Message::ChangeIp)
                    .style(text_input_style);

                let change_port =
                    iced::widget::TextInput::<Message>::new(ip.port.as_str(), &ip.port)
                        .padding(12)
                        .size(15)
                        .width(Length::FillPortion(1))
                        .on_submit(Message::ChangePort(ip.port.clone()))
                        .on_input(Message::ChangePort)
                        .style(text_input_style);

                let content = widget::column![
                    Space::new().height(Length::Fixed(
                        (1.0 - eased(self.page_reveal)) * 4.0
                    )),
                    container(widget::row![
                        section_title(
                            tr(language, "Advanced settings"),
                            tr(language, "Install models, change connection settings, and tune rendering.")
                        ),
                        Space::new().width(Length::Fill),
                        secondary_button(tr(language, "Back to settings"), Message::ToggleAdvancedSettings),
                    ])
                    .padding(18)
                    .width(Length::Fill)
                    .style(top_bar_style),
                    Space::new().height(Length::Fixed(14.0)),
                    container(widget::row![
                        widget::column![
                            Space::new().height(Length::Fixed(
                                (1.0 - eased(self.page_reveal * 1.12)) * 14.0
                            )),
                            settings_group_title(tr(language, "MODELS & SAFETY")),
                            Space::new().height(Length::Fixed(8.0)),
                            widget::column![
                        container(widget::column![
                            setting_label(tr(language, "System prompt"), tr(language, "Change the active prompt profile.")),
                            widget::pick_list(
                                prompts_list,
                                self.system_prompt.system_prompt.clone(),
                                Message::SystemPromptChange,
                            )
                            .padding([12, 14])
                            .text_size(14)
                            .style(pick_list_style)
                            .menu_style(pick_list_menu_style)
                            .width(Length::Fill),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::column![
                            widget::row![
                                setting_label(
                                    tr(language, "Local code checking"),
                                    tr(language, "Allow generated Python, Rust, C, C++, and C# snippets to be checked with locally installed tools.")
                                ),
                                widget::checkbox(self.code_checking_enabled)
                                    .label(tr(language, "Enabled"))
                                    .on_toggle(|_| Message::ToggleCodeChecking),
                            ],
                            Space::new().height(Length::Fixed(8.0)),
                            widget::text(tr(language, "Warning: checking invokes local compilers or interpreters on generated code. It can fail, consume resources, or cause unintended errors. Enable it only when you consent and trust the code."))
                                .size(12)
                                .color(warning()),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::column![
                            setting_label(
                                tr(language, "Install model"),
                                tr(language, "Enter an Ollama model name and press Enter.")
                            ),
                            model_install,
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                            ]
                        ]
                        .width(Length::Fill),

                        Space::new().width(Length::Fixed(14.0)),

                        widget::column![
                            Space::new().height(Length::Fixed(
                                (1.0 - eased(self.page_reveal * 1.18 - 0.08)) * 22.0
                            )),
                            settings_group_title(tr(language, "RUNTIME & CONNECTION")),
                            Space::new().height(Length::Fixed(8.0)),
                            widget::column![
                        container(widget::column![
                            setting_label(
                                tr(language, "Batch tokens"),
                                tr(language, "Tokens per visual update when fast streaming is off. Higher values reduce rendering work.")
                            ),
                            Space::new().height(Length::Fixed(10.0)),
                            widget::row![
                                widget::slider(1.0..=10.0, self.batch_tokens as f32, |value| {
                                    Message::ChangeBatchTokens(value as i32)
                                },),
                                Space::new().width(Length::Fixed(12.0)),
                                feedback_value_chip(
                                    self.batch_tokens.to_string(),
                                    accent(),
                                    self.settings_feedback(SettingsFeedbackTarget::BatchTokens),
                                ),
                            ],
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Fast streaming"),
                                tr(language, "Render as soon as the API yields output. Turn off to use token batching.")
                            ),
                            widget::checkbox(self.fast_streaming)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleFastStreaming),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Show tokens per second at bottom of message"),
                                tr(language, "Display the generation speed under each assistant reply. Measured from the model's own statistics, independent of token batching.")
                            ),
                            widget::checkbox(self.show_tokens_per_second)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleShowTokensPerSecond),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Content filtering"),
                                tr(language, "Censor offensive, profane, sexual, and severely inappropriate words with # characters.")
                            ),
                            widget::checkbox(self.app_state.filtering)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleFiltering),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::column![
                            setting_label(
                                tr(language, "Ollama address"),
                                tr(language, "Change the IP address and port used to connect to Ollama.")
                            ),
                            Space::new().height(Length::Fixed(12.0)),
                            widget::row![
                                change_ip,
                                Space::new().width(Length::Fixed(8.0)),
                                widget::text(":").size(20).color(text_muted()),
                                Space::new().width(Length::Fixed(8.0)),
                                change_port,
                            ],
                            Space::new().height(Length::Fixed(12.0)),
                            container(
                                widget::text(if language == Language::Spanish {
                                    format!(
                                        "Dirección actual: {}:{}",
                                        user_information.ip_address.ip,
                                        user_information.ip_address.port
                                    )
                                } else {
                                    format!(
                                        "Current address: {}:{}",
                                        user_information.ip_address.ip,
                                        user_information.ip_address.port
                                    )
                                })
                                .size(13)
                                .color(text_main())
                            )
                            .padding(10)
                            .style(chip_style(accent_2())),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                            ]
                        ]
                        .width(Length::Fill),
                    ])
                    .padding(18)
                    .width(Length::Fill)
                    .style(conversation_style),
                ];

                container(widget::scrollable(content).height(Length::Fill))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ellipsize_chat_title;

    #[test]
    fn chat_title_ellipsis_is_unicode_safe() {
        assert_eq!(ellipsize_chat_title("🦀 Rustaceans unite", 6), "🦀 Rus…");
    }

    #[test]
    fn chat_title_ellipsis_keeps_short_titles_unchanged() {
        assert_eq!(ellipsize_chat_title("Short title", 20), "Short title");
        assert_eq!(ellipsize_chat_title("12345678", 8), "12345678");
    }

    #[test]
    fn chat_title_ellipsis_forces_titles_onto_one_line() {
        assert_eq!(
            ellipsize_chat_title("first line\nsecond\tline", 18),
            "first line second…"
        );
        assert_eq!(ellipsize_chat_title("anything", 0), "");
    }
}
