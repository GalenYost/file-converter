//! The standard Iced text input with a pointer-accessible editing menu.
use crate::i18n::Language;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, clipboard, layout, mouse, overlay, renderer, text,
    widget::{Operation, Tree, tree},
};
use iced::widget::{overlay::menu, text_input};
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, keyboard, window,
};

#[derive(Debug, Clone, Default)]
pub struct Edit {
    pub value: Option<String>,
    pub copy: Option<String>,
}

#[derive(Clone, Copy)]
enum Command {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

#[derive(Clone, Copy)]
struct Item(Command, &'static str);

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

pub fn input<'a>(placeholder: &str, value: &'a str, language: Language) -> Element<'a, Edit> {
    let labels = match language {
        Language::English => ["Cut", "Copy", "Paste", "Select all"],
        Language::Ukrainian => ["Вирізати", "Копіювати", "Вставити", "Виділити все"],
        Language::Russian => ["Вырезать", "Копировать", "Вставить", "Выделить всё"],
    };
    let field = text_input(placeholder, value)
        .padding([6, 10])
        .width(Length::Fill)
        .on_input(|value| Edit {
            value: Some(value),
            copy: None,
        });
    Element::new(ContextInput {
        field: field.into(),
        value,
        labels,
        menu_class: <Theme as menu::Catalog>::default(),
    })
}

struct ContextInput<'a> {
    field: Element<'a, Edit>,
    value: &'a str,
    labels: [&'static str; 4],
    menu_class: <Theme as menu::Catalog>::Class<'a>,
}

#[derive(Default)]
struct State {
    anchor: Option<Point>,
    menu: menu::State,
    hovered: Option<usize>,
    items: Vec<Item>,
    paste: String,
}

type InputState = text_input::State<<Renderer as text::Renderer>::Paragraph>;

impl Widget<Edit, Theme, Renderer> for ContextInput<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.field)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.field));
    }
    fn size(&self) -> Size<Length> {
        self.field.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.field
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        op: &mut dyn Operation,
    ) {
        self.field
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, op);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.field.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.field.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Edit>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let child = &mut tree.children[0];
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
        ) && let Some(point) = cursor.position_in(layout.bounds())
        {
            let value = text_input::Value::new(self.value);
            let field = child.state.downcast_ref::<InputState>();
            let selected = field.is_focused() && field.cursor().selection(&value).is_some();
            if !selected {
                // Use the input's hit testing, including its horizontal scroll offset.
                for click in [
                    mouse::Event::ButtonPressed(mouse::Button::Left),
                    mouse::Event::ButtonReleased(mouse::Button::Left),
                ] {
                    self.field.as_widget_mut().update(
                        child,
                        &Event::Mouse(click),
                        layout,
                        cursor,
                        renderer,
                        clipboard,
                        shell,
                        viewport,
                    );
                }
            }
            state.anchor = Some(point);
            state.hovered = None;
            state.paste = clipboard
                .read(clipboard::Kind::Standard)
                .unwrap_or_default();
            state.paste.retain(|c| !c.is_control());
            let [cut, copy, paste, select_all] = self.labels;
            state.items.clear();
            if selected {
                state
                    .items
                    .extend([Item(Command::Cut, cut), Item(Command::Copy, copy)]);
            }
            state.items.push(Item(Command::Paste, paste));
            if !value.is_empty() {
                state.items.push(Item(Command::SelectAll, select_all));
            }
            shell.request_redraw();
            shell.capture_event();
            return;
        }

        if state.anchor.is_some()
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(_) | mouse::Event::WheelScrolled { .. })
                    | Event::Keyboard(keyboard::Event::KeyPressed { .. })
                    | Event::Window(window::Event::Unfocused)
            )
        {
            state.anchor = None;
            state.paste.clear();
            shell.request_redraw();
            if matches!(
                event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            ) {
                shell.capture_event();
                return;
            }
        }
        self.field.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Edit, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        let offset = state.anchor?;
        let mut position = layout.position() + translation + Vector::new(offset.x, offset.y);
        let width = 150.0_f32.min(viewport.width);
        position.x = position
            .x
            .clamp(viewport.x, viewport.x + viewport.width - width);
        let field = tree.children[0].state.downcast_mut::<InputState>();
        let menu = menu::Menu::new(
            &mut state.menu,
            &state.items,
            &mut state.hovered,
            |item| {
                state.anchor = None;
                let pasted = std::mem::take(&mut state.paste);
                let mut value = text_input::Value::new(self.value);
                let (start, end) = match field.cursor().state(&value) {
                    text_input::cursor::State::Selection { start, end } => {
                        (start.min(end), start.max(end))
                    }
                    text_input::cursor::State::Index(i) => (i, i),
                };
                let mut edit = Edit::default();
                match item.0 {
                    Command::SelectAll => field.select_all(),
                    Command::Copy => edit.copy = Some(value.select(start, end).to_string()),
                    Command::Cut => {
                        edit.copy = Some(value.select(start, end).to_string());
                        value.remove_many(start, end);
                        field.move_cursor_to(start);
                        edit.value = Some(value.to_string());
                    }
                    Command::Paste if !pasted.is_empty() => {
                        let insertion = text_input::Value::new(&pasted);
                        let caret = start + insertion.len();
                        value.remove_many(start, end);
                        value.insert_many(start, insertion);
                        field.move_cursor_to(caret);
                        edit.value = Some(value.to_string());
                    }
                    Command::Paste => {}
                }
                edit
            },
            None,
            &self.menu_class,
        )
        .width(width)
        .padding([4, 9])
        .text_size(14);
        Some(menu.overlay(position, *viewport, 0.0, Length::Shrink))
    }
}
