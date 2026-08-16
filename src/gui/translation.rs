use super::*;

/// Spanish is intentionally shipped as an experimental machine-generated
/// translation. Unknown/new strings fall back to English instead of disappearing.
pub(super) fn tr(language: Language, english: &'static str) -> &'static str {
    if language == Language::English {
        return english;
    }
    match english {
        "A polished desktop interface for chatting with local Ollama models." => {
            "Una interfaz de escritorio cuidada para conversar con modelos locales de Ollama."
        }
        "A polished desktop interface for chatting with local AI models." => {
            "Una interfaz de escritorio cuidada para conversar con modelos locales de IA."
        }
        "HELP" => "AYUDA",
        "Chat locally" => "Chat local",
        "Select one of your installed Ollama models, type a prompt, and press Enter to generate a response." => {
            "Selecciona uno de tus modelos de Ollama instalados, escribe un mensaje y pulsa Intro para generar una respuesta."
        }
        "Select a model from your inference backend, type a prompt, and press Enter to generate a response." => {
            "Selecciona un modelo de tu motor de inferencia, escribe un mensaje y pulsa Intro para generar una respuesta."
        }
        "Manage models" => "Gestionar modelos",
        "Use Advanced Settings to install models by name, change the Ollama address, or tune response rendering." => {
            "Usa la configuración avanzada para instalar modelos por nombre, cambiar la dirección de Ollama o ajustar la presentación de respuestas."
        }
        "Use Advanced Settings to choose Ollama or OpenVINO, configure its address, and tune response rendering." => {
            "Usa la configuración avanzada para elegir Ollama u OpenVINO, configurar su dirección y ajustar la presentación de respuestas."
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
        "User settings and chats are stored in your local application-data folder. Installed assets remain read-only." => {
            "Los ajustes y chats se guardan en la carpeta local de datos de la aplicación. Los recursos instalados son de solo lectura."
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
        "OpenVINO Model Server was not detected." => "No se detectó OpenVINO Model Server.",
        "Install Ollama or check your connection settings." => {
            "Instala Ollama o revisa la configuración de conexión."
        }
        "Start OpenVINO Model Server or check your connection settings." => {
            "Inicia OpenVINO Model Server o revisa la configuración de conexión."
        }
        "Install Ollama" => "Instalar Ollama",
        "Open setup guide" => "Abrir guía de configuración",
        "No models were detected." => "No se detectaron modelos.",
        "Install a model before sending prompts." => "Instala un modelo antes de enviar mensajes.",
        "Deploy a text-generation model in OpenVINO Model Server first." => {
            "Despliega primero un modelo de generación de texto en OpenVINO Model Server."
        }
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
        "Chat actions" => "Acciones del chat",
        "Cancel" => "Cancelar",
        "Delete chat" => "Eliminar chat",
        "Delete temporary chat" => "Eliminar chat temporal",
        "Collapse sidebar" => "Contraer barra lateral",
        "Open sidebar" => "Abrir barra lateral",
        "New chat" => "Nuevo chat",
        "Edit profile" => "Editar perfil",
        "Delete profile" => "Eliminar perfil",
        "Create profile" => "Crear perfil",
        "Close" => "Cerrar",
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
        "Config" => "Configurar",
        "Context" => "Contexto",
        "Max response" => "Respuesta máxima",
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
        "Code check" => "Comprobación de código",
        "Code Checking" => "Comprobación de código",
        "Code Checking also requires Local code checking in Advanced settings." => {
            "La comprobación de código también requiere activar la comprobación local de código en la configuración avanzada."
        }
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
        "Vision model is responding…" => "El modelo de visión está respondiendo…",
        "Vision response" => "Respuesta de visión",
        "Model" => "Modelo",
        "Ask about image" => "Preguntar sobre la imagen",
        "Analyze images with a vision-capable model." => {
            "Analiza imágenes con un modelo compatible con visión."
        }
        "Vision analysis" => "Análisis visual",
        "Attach an image and ask a vision-capable model to describe, classify, read, or reason about it." => {
            "Adjunta una imagen y pide a un modelo con visión que la describa, clasifique, lea o analice."
        }
        "This model can inspect images." => "Este modelo puede analizar imágenes.",
        "This model does not support image input." => "Este modelo no admite imágenes de entrada.",
        "Checking image capabilities…" => "Comprobando capacidades de imagen…",
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
        "Choose the backend model used for new responses." => {
            "Elige el modelo del motor para las respuestas nuevas."
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
        "Chat font" => "Fuente del chat",
        "Choose the font family used for prompts, responses, and reasoning." => {
            "Elige la familia tipográfica usada para mensajes, respuestas y razonamiento."
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
        "Manage OpenVINO models" => "Gestionar modelos de OpenVINO",
        "Models are deployed by OpenVINO Model Server. Locoryn discovers every model exposed by its /v3/models endpoint." => {
            "Los modelos se despliegan mediante OpenVINO Model Server. Locoryn detecta todos los modelos expuestos por su endpoint /v3/models."
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
        "Display the generation speed under each assistant reply. Uses backend timing when available and otherwise times the generated stream." => {
            "Muestra la velocidad de generación bajo cada respuesta. Usa la medición del servidor cuando está disponible y, en caso contrario, mide el flujo generado."
        }
        "Content filtering" => "Filtro de contenido",
        "Censor offensive, profane, sexual, and severely inappropriate words with # characters." => {
            "Censura palabras ofensivas, malsonantes, sexuales y gravemente inapropiadas con caracteres #."
        }
        "Show info popup on startup" => "Mostrar popup informativo al inicio",
        "Display the informational overview when the app launches. You can still open it anytime from the info button." => {
            "Muestra la descripción informativa cuando se inicia la app. Puedes abrirla en cualquier momento desde el botón de información."
        }
        "Ollama address" => "Dirección de Ollama",
        "OpenVINO Model Server address" => "Dirección de OpenVINO Model Server",
        "Inference backend" => "Motor de inferencia",
        "Choose the server API used for model discovery and generation. The client does not depend on local hardware architecture." => {
            "Elige la API del servidor para descubrir modelos y generar respuestas. El cliente no depende de la arquitectura del equipo local."
        }
        "Choose HTTP or HTTPS, then enter the hostname or IP address and port used to connect to Ollama." => {
            "Elige HTTP o HTTPS e introduce el nombre de host o la dirección IP y el puerto para conectar con Ollama."
        }
        "Choose HTTP or HTTPS, then enter the hostname or IP address and port used by the selected inference server." => {
            "Elige HTTP o HTTPS e introduce el nombre de host o la dirección IP y el puerto del servidor de inferencia seleccionado."
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
pub(super) struct ThinkingChoice {
    pub(super) level: ThinkingLevel,
    pub(super) language: Language,
}

impl ThinkingChoice {
    pub(super) fn from_levels(levels: &[ThinkingLevel], language: Language) -> Vec<Self> {
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
