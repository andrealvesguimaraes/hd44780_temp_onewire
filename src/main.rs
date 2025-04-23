//! This example shows how you can use PIO to read a `DS18B20` one-wire temperature sensor.

#![no_std]
#![no_main]
use core::fmt::Write;
use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{self, InterruptHandler, Pio};
use embassy_rp::pio_programs::onewire::{PioOneWire, PioOneWireProgram};
use embassy_rp::pwm::{self, Pwm};
use embassy_time::Timer;
//use embedded_hal_1::digital::OutputPin;
use heapless::String;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut pio = Pio::new(p.PIO0, Irqs);

    let prg = PioOneWireProgram::new(&mut pio.common);
    let onewire = PioOneWire::new(&mut pio.common, pio.sm0, p.PIN_2, &prg);

    let mut sensor = Ds18b20::new(onewire);

    //Configure LCD Brilho
    let _pwm = Pwm::new_output_b(p.PWM_SLICE7, p.PIN_15, {
        let mut c = pwm::Config::default();
        c.divider = 125.into();
        c.top = 200;
        c.compare_b = 50;
        c
    });

    // Configure os pinos do LCD
    //let _bright_pin = Output::new(p.PIN_15, Level::Low);
    let _rw_pin = Output::new(p.PIN_9, Level::Low);
    let rs_pin = Output::new(p.PIN_8, Level::Low);
    let en_pin = Output::new(p.PIN_10, Level::Low);
    let d4_pin = Output::new(p.PIN_11, Level::Low);
    let d5_pin = Output::new(p.PIN_12, Level::Low);
    let d6_pin = Output::new(p.PIN_13, Level::Low);
    let d7_pin = Output::new(p.PIN_14, Level::Low);

    let mut led_pin = Output::new(p.PIN_25, Level::Low);
    // Inicialize o driver HD44780 no modo de 4 bits
    let mut lcd = hd44780_driver::HD44780::new_4bit(
        rs_pin,
        en_pin,
        d4_pin,
        d5_pin,
        d6_pin,
        d7_pin,
        &mut embassy_time::Delay, // Use embassy_time::Delay para delays assíncronos
    )
    .unwrap();

    loop {
        led_pin.set_low();
        Timer::after_secs(1).await;
        led_pin.set_high();
        // Clear the screen
        lcd.reset(&mut embassy_time::Delay).unwrap();
        lcd.clear(&mut embassy_time::Delay).unwrap();

        // Write to the top line
        lcd.write_str("## TINKERBELL ##", &mut embassy_time::Delay)
            .unwrap();
        //lcd.write_str("rp-hal on", &mut embassy_time::Delay)
        //.uwrite_strnwrap();

        // Move the cursor
        lcd.set_cursor_pos(40, &mut embassy_time::Delay).unwrap();

        // Write more more text
        //lcd.write_str("HD44780! ", &mut embassy_time::Delay)
        //.unwrap();

        lcd.set_autoscroll(false, &mut embassy_time::Delay).unwrap();
        sensor.start().await; // Start a new measurement
        Timer::after_secs(1).await; // Allow 1s for the measurement to finish

        match sensor.temperature().await {
            Ok(temp) => {
                // Cria um buffer de string com capacidade fixa (e.g., 16 bytes)
                let mut buffer: String<16> = String::new();

                // Formata o valor float 'temp' para a string 'buffer'
                // "{:.1}" formata com uma casa decimal. Ajuste se necessário.
                write!(buffer, "Temp: {:.1}", temp).expect("Failed to format temperature");

                // Escreve a string formatada no LCD
                lcd.write_str(buffer.as_str(), &mut embassy_time::Delay)
                    .unwrap();
                lcd.write_byte(0xDF, &mut embassy_time::Delay).unwrap();
                lcd.write_byte(b'C', &mut embassy_time::Delay).unwrap();
            }
            _ => info!("Error!!!"),
        }
    }
}

/// DS18B20 temperature sensor driver
pub struct Ds18b20<'d, PIO: pio::Instance, const SM: usize> {
    wire: PioOneWire<'d, PIO, SM>,
}

impl<'d, PIO: pio::Instance, const SM: usize> Ds18b20<'d, PIO, SM> {
    pub fn new(wire: PioOneWire<'d, PIO, SM>) -> Self {
        Self { wire }
    }

    /// Calculate CRC8 of the data
    fn crc8(data: &[u8]) -> u8 {
        let mut temp;
        let mut data_byte;
        let mut crc = 0;
        for b in data {
            data_byte = *b;
            for _ in 0..8 {
                temp = (crc ^ data_byte) & 0x01;
                crc >>= 1;
                if temp != 0 {
                    crc ^= 0x8C;
                }
                data_byte >>= 1;
            }
        }
        crc
    }

    /// Start a new measurement. Allow at least 1000ms before getting `temperature`.
    pub async fn start(&mut self) {
        self.wire.write_bytes(&[0xCC, 0x44]).await;
    }

    /// Read the temperature. Ensure >1000ms has passed since `start` before calling this.
    pub async fn temperature(&mut self) -> Result<f32, ()> {
        self.wire.write_bytes(&[0xCC, 0xBE]).await;
        let mut data = [0; 9];
        self.wire.read_bytes(&mut data).await;
        match Self::crc8(&data) == 0 {
            true => Ok(((data[1] as u32) << 8 | data[0] as u32) as f32 / 16.),
            false => Err(()),
        }
    }
}
