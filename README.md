## Masonry

Masonry is a framework that aims to provide the foundation for Rust GUI libraries.

Masonry gives you a platform to create windows (using Glazier as a backend) each with a tree of widgets. It also gives you tools to inspect that widget tree at runtime, write unit tests on it, and generally have an easier time debugging and maintaining your app.

The framework is not opinionated about what your user-facing abstraction will be: you can implement immediate-mode GUI, the Elm architecture, functional reactive GUI, etc, on top of Masonry.

This project was originally was originally a fork of Druid that emerged from discussions I had with Raph Levien and Colin Rofls about what it would look like to turn Druid into a foundational library.

## Installing

```sh
cargo add masonry
```

### Linux

On Linux, Masonry requires gtk+3; see [GTK installation page].
(On ubuntu-based distro, running `sudo apt-get install libgtk-3-dev` from the terminal will do the job.)

### OpenBSD

On OpenBSD, Masonry requires gtk+3; install from packages:
```sh
pkg_add gtk+3
```

## Example

The todo-list example looks like this:

```rust
use masonry::widget::{prelude::*, TextBox};
use masonry::widget::{Button, Flex, Label, Portal, WidgetMut};
use masonry::Action;
use masonry::{AppDelegate, AppLauncher, DelegateCtx, WindowDescription, WindowId};

const VERTICAL_WIDGET_SPACING: f64 = 20.0;

struct Delegate {
    next_task: String,
}

impl AppDelegate for Delegate {
    fn on_action(
        &mut self,
        ctx: &mut DelegateCtx,
        _window_id: WindowId,
        _widget_id: WidgetId,
        action: Action,
        _env: &Env,
    ) {
        match action {
            Action::ButtonPressed => {
                let mut root: WidgetMut<Portal<Flex>> = ctx.get_root();
                let mut flex = root.child_mut();
                flex.add_child(Label::new(self.next_task.clone()));
            }
            Action::TextChanged(new_text) => {
                self.next_task = new_text.clone();
            }
            _ => {}
        }
    }
}

fn main() {
    // The main button with some space below, all inside a scrollable area.
    let root_widget = Portal::new(
        Flex::column()
            .with_child(
                Flex::row()
                    .with_child(TextBox::new(""))
                    .with_child(Button::new("Add task")),
            )
            .with_spacer(VERTICAL_WIDGET_SPACING),
    );

    let main_window = WindowDescription::new(root_widget)
        .title("To-do list")
        .window_size((400.0, 400.0));

    AppLauncher::with_window(main_window)
        .with_delegate(Delegate {
            next_task: String::new(),
        })
        .log_to_console()
        .launch()
        .expect("Failed to launch application");
}
```

As you can see, compared to crates like Druid or Iced, Masonry takes a fairly low-level approach to GUI: there is no complex reconciliation logic or dataflow going on behind the scenes; if you want to add a widget to the flex container, you call `flex.add_child(your_widget)`.

This simplicity makes Masonry somewhat painful if you want to use it to actually build GUI applications. The hope is that, by being low-level and straightforward, developers can easily build GUI frameworks on top of it.

(Well, in theory. The first stress-test will be porting [Panoramix](https://github.com/PoignardAzur/panoramix), a React-style GUI in Rust, to Masonry.)


## Widget Inspector

Masonry provides a UI for inspecting the Widget tree, inspired by browser devtools. It's still rudimentary for now, the hope being to eventually provide best-in-class tools for debugging all of your UI.

[TODO screenshot]

In the current inspector, you can see the Widget hierarchy, move your mouse around on the screen and select the matching Widget in the hierarchy, and get (very basic) information about the widget.


## Unit tests

Masonry is designed to make unit tests easy to write, as if the test function were a mouse-and-keyboard user. Tests look like this:

```rust
#[test]
fn some_test_with_a_button() {
    let [button_id] = widget_ids();
    let widget = Button::new("Hello").with_id(button_id);

    let mut harness = TestHarness::create(widget);

    // Make a snapshot test of the visual contents of the window
    assert_render_snapshot!(harness, "hello");

    harness.edit_root_widget(|mut button, _| {
        let mut button = button.downcast::<Button>().unwrap();
        button.set_text("World");
    });

    // Make new snapshot test now that the window has changed
    assert_render_snapshot!(harness, "world");

    // References to widget automatically implement Debug, and
    // will print their part of the widget hierarchy.
    println!("Window contents: {:?}", harness.root_widget());

    // You can also use insta to snapshot-test the widget hierarchy
    assert_debug_snapshot!(harness.root_widget());

    // Clicking on a button will produce a "ButtonPressed" action.
    harness.mouse_click_on(button_id);
    assert_eq!(
        harness.pop_action(),
        Some((Action::ButtonPressed, button_id))
    );
}
```

## Contributing

Issues and PRs are welcome. See [help-wanted issues](https://github.com/PoignardAzur/masonry-rs/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22) if you don't know where to begin.

## Roadmap

TODO

See [ROADMAP.md].