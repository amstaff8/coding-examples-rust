/*
 Author: Alan Pipitone
 Description: Rust project for the NUCLEO-F303RE that toggles a green LED 
              every second using TIM2 and interrupts. 
              Pressing the user button starts or stops the timer.
 Date: 28/05/2025
 Email: alan.pipitone@gmail.com
*/

#![no_main]
#![no_std]

use core::cell::RefCell;

use panic_semihosting as _;

use cortex_m::{asm, peripheral::NVIC};
use cortex_m_rt::entry;
use critical_section::Mutex;

use stm32f3xx_hal::{
    gpio::{self, Edge, Input, Output, PushPull},
    interrupt, pac,
    prelude::*,
    timer
};

//use core::sync::atomic::{AtomicBool, Ordering};

use rtt_target::{rprintln, rtt_init_print};

type LedPin = gpio::PA5<Output<PushPull>>;
static LED: Mutex<RefCell<Option<LedPin>>> = Mutex::new(RefCell::new(None));

type ButtonPin = gpio::PC13<Input>;
static BUTTON: Mutex<RefCell<Option<ButtonPin>>> = Mutex::new(RefCell::new(None));

type Timer2 = timer::Timer<pac::TIM2>;
static TIMER_TIM2: Mutex<RefCell<Option<Timer2>>> = Mutex::new(RefCell::new(None));

//static TIMER_TIM2_STATE: AtomicBool = AtomicBool::new(false);
static TIMER_TIM2_STATE: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));

// When the user button is pressed, the TIM2 timer is enabled.
// The timer triggers an interrupt every second.
// In the interrupt handler, the green LED is toggled.
#[entry]
fn main() -> ! {
    rtt_init_print!();
    // Getting access to registers we will need for configuration.
    let device_peripherals = pac::Peripherals::take().unwrap();
    let mut rcc = device_peripherals.RCC.constrain();
    let mut flash = device_peripherals.FLASH.constrain();

    // Configure the clocks:    
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .bypass_hse() // Bypass the HSE oscillator, assuming it's already stable.
        .sysclk(72.MHz())
        .pclk1(36.MHz())
        .pclk2(72.MHz())
        .freeze(&mut flash.acr);

    assert!(clocks.pclk1() <= 36.MHz());
    assert!(clocks.pclk2() <= 72.MHz());
    assert!(clocks.sysclk() <= 72.MHz());

    // Configure the SYSCFG and EXTI peripherals.
    // This is needed to configure the user button to trigger an interrupt.
    let mut syscfg = device_peripherals.SYSCFG.constrain(&mut rcc.apb2);
    let mut exti = device_peripherals.EXTI;

    // Split the GPIO ports to get access to the pins.
    let mut gpioc = device_peripherals.GPIOC.split(&mut rcc.ahb);
    let mut gpioa = device_peripherals.GPIOA.split(&mut rcc.ahb);

    let led = gpioa
        .pa5
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

    // Move the ownership of the led to the global LED
    critical_section::with(|cs| *LED.borrow(cs).borrow_mut() = Some(led));

    // Configuring the user button to trigger an interrupt when the button is pressed.
    let mut user_button = gpioc
        .pc13
        .into_floating_input(&mut gpioc.moder, &mut gpioc.pupdr);

    syscfg.select_exti_interrupt_source(&user_button);
    user_button.trigger_on_edge(&mut exti, Edge::Falling);
    user_button.enable_interrupt(&mut exti);

    unsafe {
        NVIC::unmask(user_button.interrupt())
    };

    // Moving ownership to the global BUTTON so we can clear the interrupt pending bit.
    critical_section::with(|cs| *BUTTON.borrow(cs).borrow_mut() = Some(user_button));

    // Configure the timer TIM2 to trigger an interrupt when it reaches the update event.
    let mut timer2 = timer::Timer::new(device_peripherals.TIM2, clocks, &mut rcc.apb1);
    timer2.enable_interrupt(timer::Event::Update);

    unsafe {
        NVIC::unmask(timer2.interrupt())
    };

    // Put the timer in the global context.
    critical_section::with(|cs| *TIMER_TIM2.borrow(cs).borrow_mut() = Some(timer2));


    rprintln!("Looping, waiting for button press...");
    loop {
        asm::wfi();
    }
}

// Button Pressed interrupt.
// The exti# maps to the pin number that is being used as an external interrupt.
// See page 291 of the stm32f303 reference manual for proof:
// http://www.st.com/resource/en/reference_manual/dm00043574.pdf
//
// This may be called more than once per button press from the user since the button may not be debounced.
#[interrupt]
fn EXTI15_10() {
    critical_section::with(|cs| {

        rprintln!("Inside interrupt handler for EXTI15_10");

        let mut button_ref = BUTTON.borrow(cs).borrow_mut();
        
        if let Some(button) = button_ref.as_mut() {

            // EXTI15_10 handles external interrupts from EXTI lines 10 to 15.
            // PC13 is connected to EXTI13, but other GPIOs may be mapped to lines 10–15 as well.
            // Therefore, we check if our button (on PC13) is the source of the interrupt.
            if button.is_interrupt_pending() {

                // Clear the interrupt pending bit so we don't infinitely call this routine
                button.clear_interrupt();

                // Toggle the state of the timer and LED using Mutex<RefCell<bool>>.
                let was_running = {
                    let mut state_ref = TIMER_TIM2_STATE.borrow(cs).borrow_mut();
                    let current = *state_ref;
                    *state_ref = !current;
                    current
                };

                let mut timer_ref = TIMER_TIM2.borrow(cs).borrow_mut();
                let timer = timer_ref.as_mut().unwrap();

                let mut led_ref = LED.borrow(cs).borrow_mut();
                let led = led_ref.as_mut().unwrap();

                if was_running {
                    rprintln!("Stopping timer and turning off LED");
                    timer.stop();
                    led.set_low().unwrap();
                } else {
                    rprintln!("Starting timer and turning on LED");
                    timer.start(1000.milliseconds());
                    led.set_high().unwrap();
                }

            }
        }


    });

}


// Interrupt handler for the timer TIM2.
// This will be called every time the timer reaches the update event.
#[interrupt]
fn TIM2() {
    // Just handle the pending interrupt event.
    critical_section::with(|cs| {

        rprintln!("Inside interrupt handler for TIM2");

        // Clear the interrupt pending bit so we don't infinitely call this routine.
        TIMER_TIM2
            .borrow(cs)
            .borrow_mut()
            .as_mut()
            .unwrap()
            .clear_event(timer::Event::Update);

        // Toggle the LED state.
        LED.borrow(cs)
            .borrow_mut()
            .as_mut()
            .unwrap()
            .toggle()
            .unwrap();

        rprintln!("Toggling LED state");
    })
}

// To run the example manually, use the following command:
// probe-rs run --chip STM32F303RETx target/thumbv7em-none-eabihf/release/stm32f303re --connect-under-reset
//
// However, this command is already configured in `.vscode/launch.json`
// and will run automatically when you press F5 in VSCode.
//
// To avoid debugger issues (e.g., "JtagNoDeviceConnected"), make sure to use
// the `--connect-under-reset` flag with probe-rs, or set 
// `"connectUnderReset": true` in `.vscode/launch.json`.
