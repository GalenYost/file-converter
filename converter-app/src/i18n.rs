use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Ukrainian,
    Russian,
}

impl Language {
    pub const ALL: &'static [Language] = &[Language::English, Language::Ukrainian, Language::Russian];

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Ukrainian => "Українська",
            Language::Russian => "Русский",
        }
    }

    pub fn app_title(&self) -> &'static str {
        match self {
            Language::English => "File converter",
            Language::Ukrainian => "File converter",
            Language::Russian => "File converter",
        }
    }

    pub fn files_in_queue(&self, count: usize) -> String {
        match self {
            Language::English => format!("{} files in queue", count),
            Language::Ukrainian => {
                let word = if count % 10 == 1 && count % 100 != 11 {
                    "файл"
                } else if (2..=4).contains(&(count % 10)) && !(12..=14).contains(&(count % 100)) {
                    "файли"
                } else {
                    "файлів"
                };
                format!("{} {} у черзі", count, word)
            }
            Language::Russian => {
                let word = if count % 10 == 1 && count % 100 != 11 {
                    "файл"
                } else if (2..=4).contains(&(count % 10)) && !(12..=14).contains(&(count % 100)) {
                    "файла"
                } else {
                    "файлов"
                };
                format!("{} {} в очереди", count, word)
            }
        }
    }

    pub fn add_files(&self) -> &'static str {
        match self {
            Language::English => "+ Add Files",
            Language::Ukrainian => "+ Додати файли",
            Language::Russian => "+ Добавить файлы",
        }
    }

    pub fn convert_to(&self) -> &'static str {
        match self {
            Language::English => "To:",
            Language::Ukrainian => "В:",
            Language::Russian => "В:",
        }
    }

    pub fn output_folder(&self) -> &'static str {
        match self {
            Language::English => "Output:",
            Language::Ukrainian => "Папка:",
            Language::Russian => "Папка:",
        }
    }

    pub fn convert_all(&self) -> &'static str {
        match self {
            Language::English => "▶ Convert All",
            Language::Ukrainian => "▶ Конвертувати всі",
            Language::Russian => "▶ Конвертировать все",
        }
    }

    pub fn clear_finished(&self) -> &'static str {
        match self {
            Language::English => "Clear",
            Language::Ukrainian => "Очистити",
            Language::Russian => "Очистить",
        }
    }

    pub fn settings_btn(&self) -> &'static str {
        "⚙"
    }

    pub fn settings_title(&self) -> &'static str {
        match self {
            Language::English => "Settings",
            Language::Ukrainian => "Налаштування",
            Language::Russian => "Настройки",
        }
    }

    pub fn language_label(&self) -> &'static str {
        match self {
            Language::English => "Language:",
            Language::Ukrainian => "Мова інтерфейсу:",
            Language::Russian => "Язык интерфейса:",
        }
    }

    pub fn concurrency_label(&self) -> &'static str {
        match self {
            Language::English => "Parallel conversions:",
            Language::Ukrainian => "Паралельні конвертації:",
            Language::Russian => "Параллельные задачи:",
        }
    }

    pub fn default_format_label(&self) -> &'static str {
        match self {
            Language::English => "Default target format:",
            Language::Ukrainian => "Формат за замовчуванням:",
            Language::Russian => "Формат по умолчанию:",
        }
    }

    pub fn ui_scale_label(&self) -> &'static str {
        match self {
            Language::English => "UI Scale:",
            Language::Ukrainian => "Масштаб інтерфейсу:",
            Language::Russian => "Масштаб интерфейса:",
        }
    }

    pub fn ui_scale_auto_btn(&self) -> &'static str {
        match self {
            Language::English => "Auto",
            Language::Ukrainian => "Авто",
            Language::Russian => "Авто",
        }
    }

    pub fn close_btn(&self) -> &'static str {
        match self {
            Language::English => "Back to Queue",
            Language::Ukrainian => "Назад до черги",
            Language::Russian => "Назад к очереди",
        }
    }

    pub fn no_files_title(&self) -> &'static str {
        match self {
            Language::English => "No files in queue",
            Language::Ukrainian => "Черга порожня",
            Language::Russian => "Очередь пуста",
        }
    }

    pub fn no_files_subtitle(&self) -> &'static str {
        match self {
            Language::English => "Click \"+ Add Files\" to choose videos, audio, or images",
            Language::Ukrainian => "Натисніть \"+ Додати файли\", щоб обрати медіа",
            Language::Russian => "Нажмите \"+ Добавить файлы\", чтобы выбрать медиа",
        }
    }

    pub fn status_queued(&self) -> &'static str {
        match self {
            Language::English => "Ready in queue",
            Language::Ukrainian => "Очікує в черзі",
            Language::Russian => "Ожидает в очереди",
        }
    }

    pub fn status_probing(&self) -> &'static str {
        match self {
            Language::English => "Probing...",
            Language::Ukrainian => "Аналіз файлу...",
            Language::Russian => "Анализ файла...",
        }
    }

    pub fn status_completed(&self) -> &'static str {
        match self {
            Language::English => "Completed ✓",
            Language::Ukrainian => "Завершено ✓",
            Language::Russian => "Завершено ✓",
        }
    }

    pub fn status_cancelled(&self) -> &'static str {
        match self {
            Language::English => "Cancelled",
            Language::Ukrainian => "Скасовано",
            Language::Russian => "Отменено",
        }
    }

    pub fn status_failed(&self, err: &str) -> String {
        match self {
            Language::English => format!("Failed: {}", err),
            Language::Ukrainian => format!("Помилка: {}", err),
            Language::Russian => format!("Ошибка: {}", err),
        }
    }

    pub fn btn_start(&self) -> &'static str {
        match self {
            Language::English => "Start",
            Language::Ukrainian => "Старт",
            Language::Russian => "Старт",
        }
    }

    pub fn btn_cancel(&self) -> &'static str {
        match self {
            Language::English => "Cancel",
            Language::Ukrainian => "Скасувати",
            Language::Russian => "Отмена",
        }
    }

    pub fn btn_remove(&self) -> &'static str {
        match self {
            Language::English => "Remove",
            Language::Ukrainian => "Видалити",
            Language::Russian => "Удалить",
        }
    }

    pub fn eta_label(&self, seconds: u64) -> String {
        match self {
            Language::English => format!("ETA: {}s", seconds),
            Language::Ukrainian => format!("Залишилось: {}с", seconds),
            Language::Russian => format!("Осталось: {}с", seconds),
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translations_coverage() {
        for lang in Language::ALL {
            assert!(!lang.add_files().is_empty());
            assert!(!lang.convert_all().is_empty());
            assert!(!lang.settings_title().is_empty());
            assert!(!lang.files_in_queue(5).is_empty());
        }
    }
}
