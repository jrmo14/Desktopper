use gpio_cdev::EventType::{FallingEdge, RisingEdge};
use gpio_cdev::{Chip, EventRequestFlags, EventType, LineEventHandle, LineRequestFlags};
use nix::poll::{poll, PollFd, PollFlags};
use parking_lot::Mutex;
use serde::export::Formatter;
use std::fmt::Display;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};

pub const RELEASED: Option<EventType> = Some(FallingEdge);
pub const HELD: Option<EventType> = Some(RisingEdge);
pub const OPEN: Option<EventType> = None;

#[derive(Clone)]
pub struct Buttons {
    pub mode: Button,
    pub cycle: Button,
    pub f0: Button,
    pub f1: Button,
    pub f2: Button,
}

impl Display for Buttons {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let extract = |state: &Option<EventType>| -> String {
            match state {
                Some(state) => match state {
                    EventType::FallingEdge => String::from("Released"),
                    EventType::RisingEdge => String::from("Held"),
                },
                None => String::from("Open"),
            }
        };
        write!(
            f,
            "Buttons:\n\tmode: ({}::{})\n\tcycle: ({}::{})\n\tf0: ({}::{})\n\tf1: ({}::{})\n\tf2: ({}::{})",
            self.mode.pin_num,
            extract(&self.mode.state),
            self.cycle.pin_num,
            extract(&self.cycle.state),
            self.f0.pin_num,
            extract(&self.f0.state),
            self.f1.pin_num,
            extract(&self.f1.state),
            self.f2.pin_num,
            extract(&self.f2.state)
        )
    }
}

// There are 3 states, Held (Rising Edge), Just released (Falling Edge), Released (None)
pub struct Button {
    pub pin_num: u32,
    pub state: Option<EventType>,
}

impl Button {
    pub fn new(pin_num: u32) -> Self {
        Button {
            pin_num,
            state: None,
        }
    }
}

impl Buttons {
    pub fn new(mode_pin: u32, cycle_pin: u32, f0_pin: u32, f1_pin: u32, f2_pin: u32) -> Self {
        Buttons {
            mode: Button::new(mode_pin),
            cycle: Button::new(cycle_pin),
            f0: Button::new(f0_pin),
            f1: Button::new(f1_pin),
            f2: Button::new(f2_pin),
        }
    }
}

impl Clone for Button {
    fn clone(&self) -> Self {
        Button {
            pin_num: self.pin_num,
            state: match &self.state {
                Some(evt_type) => match evt_type {
                    EventType::FallingEdge => Some(EventType::FallingEdge),
                    EventType::RisingEdge => Some(EventType::RisingEdge),
                },
                None => None,
            },
        }
    }
}

pub struct InputHandler {
    internal: Arc<Mutex<InputHandlerInternal>>,
}

struct InputHandlerInternal {
    buttons: Buttons,
    messenger: Sender<Buttons>,
    line_handles: Vec<LineEventHandle>,
    line_fds: Vec<PollFd>,
}

impl InputHandler {
    pub fn new(
        chip: &mut Chip,
        messenger: Sender<Buttons>,
        mode_pin: u32,
        cycle_pin: u32,
        f0_pin: u32,
        f1_pin: u32,
        f2_pin: u32,
    ) -> Self {
        let mut line_handles = Vec::new();
        let mut line_fds = Vec::new();
        // Build up the handles and fd's
        InputHandler::create_handle_fd(chip, mode_pin, &mut line_fds, &mut line_handles);

        InputHandler::create_handle_fd(chip, cycle_pin, &mut line_fds, &mut line_handles);

        InputHandler::create_handle_fd(chip, f0_pin, &mut line_fds, &mut line_handles);

        InputHandler::create_handle_fd(chip, f1_pin, &mut line_fds, &mut line_handles);

        InputHandler::create_handle_fd(chip, f2_pin, &mut line_fds, &mut line_handles);

        // Store everything in an internal struct so we can chuck it off into the worker thread ez pz
        let internal = Arc::from(Mutex::from(InputHandlerInternal {
            buttons: Buttons::new(mode_pin, cycle_pin, f0_pin, f1_pin, f2_pin),
            messenger,
            line_handles,
            line_fds,
        }));

        InputHandler { internal }
    }

    pub fn start(&mut self) -> JoinHandle<()> {
        // Get something we can move into the thread
        let internal_clone = self.internal.clone();
        spawn(move || {
            // Just lock down the internal struct, nobody else is going to be using it
            let mut internal = internal_clone.lock();
            loop {
                match poll(&mut internal.line_fds, -1) {
                    Ok(timeout_status) => {
                        if timeout_status == 0 {
                            warn!("Input loop timeout!?!?!?")
                        } else {
                            for i in 0..internal.line_fds.len() {
                                if let Some(evts) = internal.line_fds[i].revents() {
                                    let handle = internal.line_handles.get(i).unwrap();
                                    let offset = handle.line().offset();
                                    // We've got some kinda event
                                    if evts.contains(PollFlags::POLLIN) {
                                        match handle.get_event() {
                                            Ok(event) => {
                                                // Update the appropriate button's state
                                                if offset == internal.buttons.mode.pin_num {
                                                    internal.buttons.mode.state =
                                                        Some(event.event_type());
                                                } else if offset == internal.buttons.cycle.pin_num {
                                                    internal.buttons.cycle.state =
                                                        Some(event.event_type());
                                                } else if offset == internal.buttons.f0.pin_num {
                                                    internal.buttons.f0.state =
                                                        Some(event.event_type());
                                                } else if offset == internal.buttons.f1.pin_num {
                                                    internal.buttons.f1.state =
                                                        Some(event.event_type());
                                                } else if offset == internal.buttons.f2.pin_num {
                                                    internal.buttons.f2.state =
                                                        Some(event.event_type());
                                                }
                                                // A button was released
                                                if event.event_type() == FallingEdge {
                                                    info!("Sending buttons: {}", internal.buttons);
                                                    // Send all the button states to the listener
                                                    if let Err(e) = internal
                                                        .messenger
                                                        .send(internal.buttons.clone())
                                                    {
                                                        error!("Failed to send buttons, {}", e)
                                                    }
                                                    // Reset the button states after one is released
                                                    internal.buttons.mode.state = None;
                                                    internal.buttons.cycle.state = None;
                                                    internal.buttons.f0.state = None;
                                                    internal.buttons.f1.state = None;
                                                    internal.buttons.f2.state = None;
                                                }
                                            }
                                            Err(e) => error!(
                                                "Handle {} failed to unwrap event :: {}",
                                                offset, e
                                            ),
                                        }
                                    } else if evts.contains(PollFlags::POLLPRI) {
                                        // Don't really know how this would happen let's keep track though
                                        warn!("[{}] Got a POLLPRI", handle.line().offset())
                                    }
                                }
                            }
                        }
                    }
                    // Something is wrong
                    Err(e) => error!("Poll failed to return: {}", e),
                }
            }
        })
    }

    // Builds the necessary things and puts them in the right vector for each button
    fn create_handle_fd(
        chip: &mut Chip,
        line_offset: u32,
        button_fds: &mut Vec<PollFd>,
        event_handles: &mut Vec<LineEventHandle>,
    ) {
        let line_handle = chip
            .get_line(line_offset)
            .unwrap()
            .events(
                LineRequestFlags::INPUT,
                EventRequestFlags::BOTH_EDGES,
                "desktopper",
            )
            .unwrap();
        button_fds.push(PollFd::new(
            (&line_handle).as_raw_fd(),
            PollFlags::POLLIN | PollFlags::POLLPRI,
        ));
        event_handles.push(line_handle);
    }
}
