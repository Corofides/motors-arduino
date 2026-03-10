#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

use panic_halt as _;
use avr_device::atmega328p::{
    self, PORTB, EXINT
};
use core::cell::RefCell;
use avr_device::interrupt::{CriticalSection, Mutex};

static PWM_CONTROL: Mutex<RefCell<Option<PWMControl>>> = Mutex::new(RefCell::new(None));
static BUTTON: Mutex<RefCell<Option<Button>>> = Mutex::new(RefCell::new(None));

pub struct Button {
    pub port: u8,
    pub was_high: bool,
    pub can_change: bool,
    pub on_click_handle: Option<fn(&mut PWMControl)>,
    pub on_press_handle: Option<fn(&mut PWMControl)>,
    pub on_release_handle: Option<fn(&mut PWMControl)>,
}

impl Button {
    pub fn setup(&mut self) {
        avr_device::interrupt::free(|cs| {
            let mut pwm_control = PWM_CONTROL.borrow(cs).borrow_mut();
            
            if let Some(pwm_control) = pwm_control.as_mut() {
                pwm_control.pin_control.exint.pcicr.write(|w| {
                    w.pcie().bits(self.port)
                });

                pwm_control.pin_control.exint.pcmsk0.write(|w| {
                    w.pcint().bits(self.port)
                });

            }
        });
    }
    pub fn on_interrupt(&mut self, pwm_control: &mut PWMControl) {
        if !self.can_change {
            return;
        }

        self.can_change = false;

        if !self.was_high {
            self.was_high = true;

            if let Some(on_press) = self.on_press_handle {
                (on_press)(pwm_control)
            }
            return;
        }

        self.was_high = false;

        if let Some(on_release) = self.on_release_handle {
            (on_release)(pwm_control);
        }

        if let Some(on_click) = self.on_click_handle {
            (on_click)(pwm_control);
        }

    }
    pub fn allow_change(&mut self) {
        self.can_change = true;
    }
}

#[derive(Default, Clone)]
pub enum Direction {
    #[default]
    Forward,
    Backward,
}

pub struct PWMControl {
    pub direction: Direction,
    pub forward_pin: Output,
    pub backward_pin: Output,
    pub pin_control: PortControl,
}

impl PWMControl {
    fn clear(&self) {
        self.pin_control.clear_pins();
    }
    fn switch_direction(&mut self) {
        self.pin_control.clear_pins();
        match self.direction {
            Direction::Forward => {
                self.set_direction(Direction::Backward);
            }
            Direction::Backward => {
                self.set_direction(Direction::Forward);
            }
        }
    }
    fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }
    fn pulse(&self) {
        match self.direction {
            Direction::Forward => {
                self.pin_control.toggle_pin(&self.forward_pin);
            },
            Direction::Backward => {
                self.pin_control.toggle_pin(&self.backward_pin);
            }
        }
    }
}

#[derive(PartialEq)]
pub enum Output {
    P12,
    P13,
}

pub struct PortControl {
    pub port: PORTB,
    pub exint: EXINT,
}

impl PortControl {
    pub fn clear_pins(&self) {
        self.port.portb.write(|w| w.pb5().clear_bit());
        self.port.portb.write(|w| w.pb4().clear_bit());
    }
    pub fn toggle_pin(&self, output: &Output) {
        match output {
            Output::P12 => {
                self.port.pinb.write(|w| w.pb4().set_bit());
            },
            Output::P13 => {
                self.port.pinb.write(|w| w.pb5().set_bit());
            }
        }
    }
}

#[avr_device::interrupt(atmega328p)]
fn TIMER0_OVF() {

    let cs = unsafe { CriticalSection::new() };

    let mut pwm_control = PWM_CONTROL.borrow(cs).borrow_mut();
    let mut button = BUTTON.borrow(cs).borrow_mut();

    if let Some(pwm_control) = pwm_control.as_mut() {
        pwm_control.pulse();
    }

    if let Some(button) = button.as_mut() {
        button.allow_change();
    }

}

#[avr_device::interrupt(atmega328p)]
fn PCINT0() {
     let cs = unsafe { CriticalSection::new() };

     let mut pwm_control = PWM_CONTROL.borrow(cs).borrow_mut();
     let mut button = BUTTON.borrow(cs).borrow_mut();

     let Some(button) = button.as_mut() else {
         return;
     };

     let Some(pwm_control) = pwm_control.as_mut() else {
         return;
     };

     button.on_interrupt(pwm_control);
 }


#[avr_device::entry]
fn main() -> ! {
    let dp = atmega328p::Peripherals::take().unwrap();
    
    dp.TC0.tccr0b.write(|w| {
        w.cs0().prescale_1024()
    });

    dp.TC0.timsk0.write(|w| {
        w.toie0().set_bit()
    });

    dp.PORTB.ddrb.write(|w| {
        w.pb0().clear_bit(); // Read; Pin 8;
        w.pb4().set_bit(); // Pin 12;
        w.pb5().set_bit()  // Pin 13;
    });

    dp.EXINT.pcicr.write(|w| {
        w.pcie().bits(0b001)
    });

    dp.EXINT.pcmsk0.write(|w| {
        w.pcint().bits(0b001)
    });
        
    avr_device::interrupt::free(|cs| {
        let port_control = PortControl {
            port: dp.PORTB,
            exint: dp.EXINT,
        };

        let pwm_control = PWMControl {
            direction: Direction::Forward,
            forward_pin: Output::P13,
            backward_pin: Output::P12,
            pin_control: port_control,
        };

        let on_click_handle = |pwm_control: &mut PWMControl| {
            pwm_control.switch_direction();
        };

        let on_click_handle: fn(pwm_control: &mut PWMControl) -> () = on_click_handle;

        let mut button = Button {
            port: 0b001,
            was_high: false,
            can_change: true,
            on_press_handle: None,
            on_release_handle: None,
            on_click_handle: Some(on_click_handle)
        };

        button.setup();
        pwm_control.clear();

        PWM_CONTROL.borrow(cs).replace(Some(pwm_control));
        BUTTON.borrow(cs).replace(Some(button));

    });

    unsafe {
        avr_device::interrupt::enable();
    }
    
    loop { /* Do Nothing */ }
}
