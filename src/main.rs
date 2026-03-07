#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

use panic_halt as _;
use avr_device::atmega328p::{self, PORTB};
use core::cell::RefCell;
use avr_device::interrupt::{CriticalSection, Mutex};

static PWM_CONTROL: Mutex<RefCell<Option<PWMControl>>> = Mutex::new(RefCell::new(None));

#[derive(Default, Clone)]
enum Direction {
    #[default]
    Forward,
    Backward,
}

struct PWMControl {
    pub direction: Direction,
    pub forward_pin: Output,
    pub backward_pin: Output,
    pub pin_control: PinControl,
}

impl PWMControl {
    fn clear(&self) {
        self.pin_control.clear_pins();
    }
    fn switch_direction(&mut self) {
        match self.direction {
            Direction::Forward => {
                self.direction = Direction::Backward;
            }
            Direction::Backward => {
                self.direction = Direction::Forward;
            }
        }
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
enum Output {
    P_12,
    P_13,
}

struct PinControl {
    pub port: PORTB 
}

impl PinControl {
    pub fn clear_pins(&self) {
        self.port.portb.write(|w| w.pb5().clear_bit());
        self.port.portb.write(|w| w.pb4().clear_bit());
    }
    pub fn toggle_pin(&self, output: &Output) {
        match output {
            Output::P_12 => {
                self.port.pinb.write(|w| w.pb4().set_bit());
            },
            Output::P_13 => {
                self.port.pinb.write(|w| w.pb5().set_bit());
            }
        }
    }
}

#[avr_device::interrupt(atmega328p)]
fn TIMER0_OVF() {

    let cs = unsafe { CriticalSection::new() };

    let mut pwm_control = PWM_CONTROL.borrow(cs).borrow_mut();

    if let Some(pwm_control) = pwm_control.as_mut() {
        pwm_control.pulse();
    }

}

// Remove interrupt for now.
/*
#[avr_device::interrupt(atmega328p)]
fn PCINT0() {
    let cs = unsafe { CriticalSection::new() };

    let mut pwm_control = PWM_CONTROL.borrow(cs).borrow_mut();

    if let Some(pwm_control) = pwm_control.as_mut() {
        pwm_control.switch_direction();
    }
}
*/


#[avr_device::entry]
fn main() -> ! {
    let mut number: i32 = 0;

    let dp = atmega328p::Peripherals::take().unwrap();
    
    dp.TC0.tccr0b.write(|w| {
        w.cs0().prescale_1024()
    });

    dp.TC0.timsk0.write(|w| {
        w.toie0().set_bit()
    });

    /*dp.EXINT.pcicr.write(|w| {
        w.pcie().bits(0b001)
    });

    dp.EXINT.pcmsk0.write(|w| {
        w.pcint().bits(0b001)
    });*/
    
    dp.PORTB.ddrb.write(|w| {
        w.pb0().clear_bit(); // Read; Pin 8;
        w.pb4().set_bit(); // Pin 12;
        w.pb5().set_bit()  // Pin 13;
    });

    dp.PORTB.portb.write(|w| {
        w.pb0().set_bit()
    });

    /*dp.EXINT.pcicr.write(|w| {
        w.pcie().bits(0b010)
    });

    dp.EXINT.pcmsk0.write(|w| {
        w.pcint().bits(0b010)
    });*/

        
    avr_device::interrupt::free(|cs| {
        let pin_control = PinControl {
            port: dp.PORTB,
        };

        let pwm_control = PWMControl {
            direction: Direction::Forward,
            forward_pin: Output::P_13,
            backward_pin: Output::P_12,
            pin_control: pin_control,
        };

        pwm_control.clear();

        PWM_CONTROL.borrow(cs).replace(Some(pwm_control));
    });

    // Disable all interrupts for testing.
    /*unsafe {
        avr_device::interrupt::enable();
    }*/
    
    loop { 

        avr_device::interrupt::free(|cs| {

            let mut pwm_control = PWM_CONTROL.borrow(cs).borrow_mut();

            if let Some(pwm_control) = pwm_control.as_mut() {
                let pin_control = &pwm_control.pin_control;
                let is_high = pin_control.port.pinb.read().pb0().bit_is_set();

                if !is_high {
                    pin_control.port.portb.write(|w| w.pb5().set_bit());
                } else {
                    pin_control.port.portb.write(|w| w.pb5().clear_bit());
                }
            }

        });

        
    }
}
