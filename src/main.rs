#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt_rtt as _;
use usb_device::UsbError;
use usbd_human_interface_device::UsbHidError; // global logger
                                              //use panic_probe as _;
use core::mem;
use core::{borrow::BorrowMut, cell::RefMut};
use panic_halt as _; // When a panic occurs, stop the microcontroller
use stm32f4xx_hal::adc::config::Clock;

use analog_multiplexer::{DummyPin, Multiplexer};
use stm32f4xx_hal::{
    adc::{
        config::{AdcConfig, SampleTime},
        Adc,
    },
    otg_fs::{UsbBus, UsbBusType, USB},
    pac,
    prelude::*,
};
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid};
use usbd_human_interface_device::device::keyboard::NKROBootKeyboardConfig;
use usbd_human_interface_device::usb_class::UsbHidClassBuilder;

mod layout;
use layout::{ACTUATION_THRESHOLD, IGNORE_BELOW_MV, NORTH_DOWN, RECALIBRATION_SEC, USB_PID, USB_VID};

use usbd_human_interface_device::page::Keyboard;

static mut EP_MEMORY: [u32; 1024] = [0; 1024];

const MONO_HZ: u32 = 144_000_000;
#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(25.MHz())
        .require_pll48clk()
        .sysclk(MONO_HZ.Hz())
        .hclk(MONO_HZ.Hz())
        .pclk1(36.MHz())
        .pclk2(72.MHz())
        .freeze();
    let dp = pac::Peripherals::take().unwrap();
    let gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();
    let gpioc = dp.GPIOC.split();
    /*
    let usb = stm32f4xx_hal::otg_fs::USB::new(
        (dp.OTG_FS_GLOBAL, dp.OTG_FS_DEVICE, dp.OTG_FS_PWRCLK),
        (gpioa.pa11, gpioa.pa12),
        &clocks,
    );
    unsafe {
        *USB_BUS = Some(stm32f4xx_hal::otg_fs::UsbBusType::new(usb, &mut ep_mem));
    }
    let usb_bus = USB_BUS.as_ref().unwrap();
    */
    let usb = USB::new(
        (dp.OTG_FS_GLOBAL, dp.OTG_FS_DEVICE, dp.OTG_FS_PWRCLK),
        (gpioa.pa11, gpioa.pa12),
        &clocks,
    );

    #[allow(static_mut_refs)]
    let mut usb_bus = UsbBus::new(usb, unsafe { &mut EP_MEMORY });

    //extending lifetime of usb_bus to static
    //this is sound since main never returns and therefore usb_bus stays on the stack
    let usb_bus: &'static RefMut<UsbBusAllocator<UsbBusType>> =
        unsafe { mem::transmute(&usb_bus.borrow_mut()) };
    let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(USB_VID, USB_PID))
        .device_class(0x01)
        .strings(&[StringDescriptors::default()
            .manufacturer("dickbutt")
            .product("HKB")
            .serial_number("v0")])
        .unwrap()
        .build();
    let mut keyboard = UsbHidClassBuilder::new()
        .add_device(NKROBootKeyboardConfig::default())
        .build(usb_bus);
    let adc_config = AdcConfig::default().clock(Clock::Pclk2_div_2);
    let mut adc = Adc::adc1(dp.ADC1, true, adc_config);

    let pa0 = gpioa.pa0.into_analog();
    let pa1 = gpioa.pa1.into_analog();
    let pa2 = gpioa.pa2.into_analog();
    let pa3 = gpioa.pa3.into_analog();
    let pa4 = gpioa.pa4.into_analog();

    let s0 = gpioc.pc13.into_push_pull_output();
    let s1 = gpiob.pb8.into_push_pull_output();
    let s2 = gpiob.pb12.into_push_pull_output();
    let s3 = gpioa.pa15.into_push_pull_output();
    let en = DummyPin; // Just run it to GND to keep always-enabled

    // pins to specify pin on multiplexer
    let select_pins = (s0, s1, s2, s3, en);

    // pins to read
    let analog_pins = (pa0, pa1, pa2, pa3, pa4);

    let mut multiplexer = Multiplexer::new(select_pins);
    // still fairly accurate but faster
    let sample_time = SampleTime::Cycles_15;

    // states in millivolts
    let mut keystates = [0u16; 80];
    let mut default_keystates = [0u16; 80];
    let mut previous_states = [0u16; 80];

    let cp = cortex_m::peripheral::Peripherals::take().unwrap();

    // 1khz usb
    let mut timer = cp.SYST.counter_us(&clocks);

    // used to set default values
    let mut timer_count = 0;

    loop {
        timer_count += 1;
        // 2khz polling
        timer.start(500.micros()).unwrap();
        for channel in 0..16 {
            multiplexer.set_channel(channel as u8);
            for multiplexer in 0..5 {
                let sample = match multiplexer {
                    0 => adc.convert(&analog_pins.0, sample_time),
                    1 => adc.convert(&analog_pins.1, sample_time),
                    2 => adc.convert(&analog_pins.2, sample_time),
                    3 => adc.convert(&analog_pins.3, sample_time),
                    4 => adc.convert(&analog_pins.4, sample_time),
                    _ => 0,
                };
                // Div 4
                let mut millivolts = adc.sample_to_millivolts(sample) / 4;
                if !NORTH_DOWN {
                    millivolts = 5000 - millivolts;
                }
                // save some cpu time if we don't care
                if millivolts < IGNORE_BELOW_MV {
                    continue;
                }
                let idx = multiplexer * 16 + channel;

                previous_states[idx] = keystates[idx];
                keystates[idx] = millivolts;
            }
        }

        {
            match keyboard.device().write_report(assemble_report(
                &keystates,
                &default_keystates,
                &previous_states,
            )) {
                Err(UsbHidError::WouldBlock) => {}
                Err(UsbHidError::Duplicate) => {}
                Ok(_) => {}
                Err(e) => {
                    core::panic!("Failed to write keyboard report: {:?}", e)
                }
            };
        }

        match keyboard.tick() {
            Err(UsbHidError::WouldBlock) => {}
            Ok(_) => {}
            Err(e) => {
                core::panic!("Failed to process keyboard tick: {:?}", e)
            }
        };

        if usb_dev.poll(&mut [&mut keyboard]) {
            match keyboard.device().read_report() {
                Err(UsbError::WouldBlock) => {}
                Err(e) => {
                    core::panic!("Failed to read keyboard report: {:?}", e)
                }
                Ok(_leds) => {}
            }
        }

        // reset defaults 10 sec
        if timer_count > RECALIBRATION_SEC * 1000 {
            timer_count = 0;
            for (idx, key) in keystates.iter().enumerate() {
                if *key < default_keystates[idx] + ACTUATION_THRESHOLD {
                    continue;
                }
                default_keystates[idx] = *key;
            }
        }
        nb::block!(timer.wait()).unwrap();
    }
}

fn check_mask(mask: &mut bool, idx: usize, states: &[u16], default_states: &[u16]) {
    if states[idx] >= default_states[idx] + ACTUATION_THRESHOLD {
        *mask = true;
    }
}

fn assemble_report(
    states: &[u16],
    default_states: &[u16],
    previous_states: &[u16],
) -> [Keyboard; 80] {
    unimplemented!();
}
