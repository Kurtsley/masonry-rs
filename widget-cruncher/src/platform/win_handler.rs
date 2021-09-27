// Copyright 2019 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The implementation of the WinHandler trait (druid-shell integration).

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::kurbo::Size;
use crate::piet::Piet;

use crate::{
    Env, Event, Handled, InternalEvent, KeyEvent, PlatformError, Selector, TimerToken, WindowId,
};

use crate::app_root::AppRoot;
use crate::platform::window_description::{PendingWindow, WindowConfig};

use druid_shell::WindowBuilder;
use druid_shell::{
    text::InputHandler, Application, FileDialogToken, FileInfo, IdleToken, MouseEvent, Region,
    Scale, TextFieldToken, WinHandler, WindowHandle,
};

pub(crate) const RUN_COMMANDS_TOKEN: IdleToken = IdleToken::new(1);

/// A token we are called back with if an external event was submitted.
pub(crate) const EXT_EVENT_IDLE_TOKEN: IdleToken = IdleToken::new(2);

/// The struct implements the druid-shell `WinHandler` trait.
///
/// One `DruidHandler` exists per window.
///
/// This is something of an internal detail and possibly we don't want to surface
/// it publicly.
pub struct DruidHandler {
    /// The shared app state.
    pub(crate) app_state: AppState,
    /// The id for the current window.
    window_id: WindowId,
}

/// The top level event handler.
///
/// This corresponds to the `AppHandler` trait in druid-shell, which is only
/// used to handle events that are not associated with a window.
///
/// Currently, this means only menu items on macOS when no window is open.
pub(crate) struct AppHandler {
    app_state: AppState,
}

/// State shared by all windows in the UI.
#[derive(Clone)]
pub(crate) struct AppState {
    inner: Rc<RefCell<AppRoot>>,
}

/// The information for forwarding druid-shell's file dialog reply to the right place.
pub struct DialogInfo {
    /// The window to send the command to.
    pub id: WindowId,
    /// The command to send if the dialog is accepted.
    pub accept_cmd: Selector<FileInfo>,
    /// The command to send if the dialog is cancelled.
    pub cancel_cmd: Selector<()>,
}

impl AppHandler {
    pub(crate) fn new(app_state: AppState) -> Self {
        Self { app_state }
    }
}

impl AppState {
    pub(crate) fn new(app: Application, env: Env) -> Self {
        let inner = Rc::new(RefCell::new(AppRoot {
            app,
            file_dialogs: HashMap::new(),
            menu_window: None,
            env,
            windows: Default::default(),
            ime_focus_change: None,
        }));

        AppState { inner }
    }

    pub(crate) fn app(&self) -> Application {
        self.inner.borrow().app.clone()
    }
}

impl DruidHandler {
    /// Note: the root widget doesn't go in here, because it gets added to the
    /// app state.
    pub(crate) fn new_shared(app_state: AppState, window_id: WindowId) -> DruidHandler {
        DruidHandler {
            app_state,
            window_id,
        }
    }
}

impl AppState {
    pub(crate) fn env(&self) -> Env {
        self.inner.borrow().env.clone()
    }

    pub(crate) fn add_window(&self, id: WindowId, window: PendingWindow) {
        self.inner.borrow_mut().windows.add(id, window);
    }

    fn connect_window(&mut self, window_id: WindowId, handle: WindowHandle) {
        self.inner.borrow_mut().connect(window_id, handle)
    }

    fn remove_window(&mut self, window_id: WindowId) {
        self.inner.borrow_mut().remove_window(window_id)
    }

    fn window_got_focus(&mut self, window_id: WindowId) {
        self.inner.borrow_mut().window_got_focus(window_id)
    }

    /// Send an event to the widget hierarchy.
    ///
    /// Returns `true` if the event produced an action.
    ///
    /// This is principally because in certain cases (such as keydown on Windows)
    /// the OS needs to know if an event was handled.
    fn do_window_event(&mut self, event: Event, window_id: WindowId) -> Handled {
        let result = self.inner.borrow_mut().do_window_event(window_id, event);
        //self.process_commands();
        //self.inner.borrow_mut().do_update();
        self.inner.borrow_mut().invalidate_and_finalize();
        let ime_change = self.inner.borrow_mut().ime_focus_change.take();
        if let Some(ime_change) = ime_change {
            (ime_change)()
        }
        result
    }

    fn prepare_paint_window(&mut self, window_id: WindowId) {
        self.inner.borrow_mut().prepare_paint(window_id);
    }

    fn paint_window(&mut self, window_id: WindowId, piet: &mut Piet, invalid: &Region) {
        self.inner.borrow_mut().paint(window_id, piet, invalid);
    }

    fn idle(&mut self, token: IdleToken) {
        match token {
            RUN_COMMANDS_TOKEN => {
                //self.process_commands();
                //self.inner.borrow_mut().do_update();
                self.inner.borrow_mut().invalidate_and_finalize();
            }
            EXT_EVENT_IDLE_TOKEN => {
                //self.process_ext_events();
                //self.process_commands();
                //self.inner.borrow_mut().do_update();
                self.inner.borrow_mut().invalidate_and_finalize();
            }
            other => tracing::warn!("unexpected idle token {:?}", other),
        }
    }

    /// Handle a 'command' message from druid-shell. These map to  an item
    /// in an application, window, or context (right-click) menu.
    ///
    /// If the menu is  associated with a window (the general case) then
    /// the `window_id` will be `Some(_)`, otherwise (such as if no window
    /// is open but a menu exists, as on macOS) it will be `None`.
    fn handle_system_cmd(&mut self, cmd_id: u32, window_id: Option<WindowId>) {
        todo!();
    }

    fn request_close_window(&mut self, id: WindowId) {
        self.inner.borrow_mut().request_close_window(id);
    }

    fn request_close_all_windows(&mut self) {
        self.inner.borrow_mut().request_close_all_windows();
    }

    fn show_window(&mut self, id: WindowId) {
        self.inner.borrow_mut().show_window(id);
    }

    fn release_ime_lock(&mut self, window_id: WindowId, token: TextFieldToken) {
        let needs_update = self.inner.borrow_mut().release_ime_lock(window_id, token);
        if let Some(widget) = needs_update {
            let event = Event::Internal(InternalEvent::RouteImeStateChange(widget));
            self.do_window_event(event, window_id);
        }
    }

    fn quit(&self) {
        self.inner.borrow().app.quit()
    }

    #[cfg(target_os = "macos")]
    fn hide_app(&self) {
        use druid_shell::platform::mac::ApplicationExt as _;
        self.inner.borrow().app.hide()
    }

    #[cfg(target_os = "macos")]
    fn hide_others(&mut self) {
        use druid_shell::platform::mac::ApplicationExt as _;
        self.inner.borrow().app.hide_others();
    }

    pub(crate) fn build_native_window(
        &mut self,
        id: WindowId,
        mut pending: PendingWindow,
        config: WindowConfig,
    ) -> Result<WindowHandle, PlatformError> {
        let mut builder = WindowBuilder::new(self.app());
        config.apply_to_builder(&mut builder);

        let env = self.env();

        pending.size_policy = config.size_policy;
        pending.title.resolve(&env);
        builder.set_title(pending.title.display_text().to_string());

        let handler = DruidHandler::new_shared((*self).clone(), id);
        builder.set_handler(Box::new(handler));

        self.add_window(id, pending);
        builder.build()
    }
}

impl druid_shell::AppHandler for AppHandler {
    fn command(&mut self, id: u32) {
        self.app_state.handle_system_cmd(id, None)
    }
}

impl WinHandler for DruidHandler {
    fn connect(&mut self, handle: &WindowHandle) {
        self.app_state
            .connect_window(self.window_id, handle.clone());

        let event = Event::WindowConnected;
        self.app_state.do_window_event(event, self.window_id);
    }

    fn prepare_paint(&mut self) {
        self.app_state.prepare_paint_window(self.window_id);
    }

    fn paint(&mut self, piet: &mut Piet, region: &Region) {
        self.app_state.paint_window(self.window_id, piet, region);
    }

    fn size(&mut self, size: Size) {
        let event = Event::WindowSize(size);
        self.app_state.do_window_event(event, self.window_id);
    }

    fn scale(&mut self, _scale: Scale) {
        // TODO: Do something with the scale
    }

    fn command(&mut self, id: u32) {
        self.app_state.handle_system_cmd(id, Some(self.window_id));
    }

    fn save_as(&mut self, token: FileDialogToken, file_info: Option<FileInfo>) {
        //self.app_state.handle_dialog_response(token, file_info);
    }

    fn open_file(&mut self, token: FileDialogToken, file_info: Option<FileInfo>) {
        //self.app_state.handle_dialog_response(token, file_info);
    }

    fn mouse_down(&mut self, event: &MouseEvent) {
        // TODO: double-click detection (or is this done in druid-shell?)
        let event = Event::MouseDown(event.clone().into());
        self.app_state.do_window_event(event, self.window_id);
    }

    fn mouse_up(&mut self, event: &MouseEvent) {
        let event = Event::MouseUp(event.clone().into());
        self.app_state.do_window_event(event, self.window_id);
    }

    fn mouse_move(&mut self, event: &MouseEvent) {
        let event = Event::MouseMove(event.clone().into());
        self.app_state.do_window_event(event, self.window_id);
    }

    fn mouse_leave(&mut self) {
        self.app_state
            .do_window_event(Event::Internal(InternalEvent::MouseLeave), self.window_id);
    }

    fn key_down(&mut self, event: KeyEvent) -> bool {
        self.app_state
            .do_window_event(Event::KeyDown(event), self.window_id)
            .is_handled()
    }

    fn key_up(&mut self, event: KeyEvent) {
        self.app_state
            .do_window_event(Event::KeyUp(event), self.window_id);
    }

    fn wheel(&mut self, event: &MouseEvent) {
        self.app_state
            .do_window_event(Event::Wheel(event.clone().into()), self.window_id);
    }

    fn zoom(&mut self, delta: f64) {
        let event = Event::Zoom(delta);
        self.app_state.do_window_event(event, self.window_id);
    }

    fn got_focus(&mut self) {
        self.app_state.window_got_focus(self.window_id);
    }

    fn timer(&mut self, token: TimerToken) {
        self.app_state
            .do_window_event(Event::Timer(token), self.window_id);
    }

    fn idle(&mut self, token: IdleToken) {
        self.app_state.idle(token);
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn acquire_input_lock(
        &mut self,
        token: TextFieldToken,
        mutable: bool,
    ) -> Box<dyn InputHandler> {
        self.app_state
            .inner
            .borrow_mut()
            .get_ime_lock(self.window_id, token, mutable)
    }

    fn release_input_lock(&mut self, token: TextFieldToken) {
        self.app_state.release_ime_lock(self.window_id, token);
    }

    fn request_close(&mut self) {
        /*
        self.app_state
            .handle_cmd(sys_cmd::CLOSE_WINDOW.to(self.window_id));
        self.app_state.process_commands();
        */
        //self.app_state.inner.borrow_mut().do_update();
        self.app_state.inner.borrow_mut().invalidate_and_finalize();
    }

    fn destroy(&mut self) {
        self.app_state.remove_window(self.window_id);
    }
}
