use embedded_hal::blocking::i2c::{Write, WriteRead};
use embedded_hal::digital::v2::OutputPin;
use embedded_time::duration::Milliseconds;
use embedded_time::Clock;

const AVAILABLE_STORAGE: usize = 64_000;
const PAGESIZE: usize = 128;
const DEFAULT_ADDRESS: u8 = 0x50;

///Errors
pub enum Error<E : Write + WriteRead> {
    OutOfRange,
    TooMuchData,
    I2cWriteRead(<E as WriteRead>::Error),
    I2cWrite(<E as Write>::Error),
}

pub struct Eeprom<'a, I2C : Write + WriteRead, WP: OutputPin, CLOCK: Clock> {
    address: u8,
    i2c: I2C,
    wp: WP,
    clock: &'a CLOCK,
}

impl<'a, I2C: Write + WriteRead, WP: OutputPin, CLOCK: Clock> Eeprom<'a, I2C, WP, CLOCK> {
    pub fn new(i2c : I2C, wp: WP, clock: &'a CLOCK) -> Self {
        Eeprom {
            i2c,
            address: DEFAULT_ADDRESS,
            wp,
            clock,
        }
    }

    fn with_wp_low<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.wp.set_low().ok();
        let result = f(self);
        self.wp.set_high().ok();

        result
    }

    //TODO: AFAIK STM32 I2C modules do not allow proper ack polling, so I need to replace it with an adequately long delay
    pub fn ack_polling(&mut self) -> Result<(), Error<I2C>> {
        self.clock
            .new_timer(Milliseconds::new(5))
            .start()
            .ok()
            .unwrap()
            .wait()
            .ok()
            .unwrap();

        Ok(())
    }

    pub fn write_byte(
        &mut self,
        addr: u16,
        byte: u8,
    ) -> Result<(), Error<I2C>> {
        if addr as usize > AVAILABLE_STORAGE {
            return Err(Error::OutOfRange);
        }

        if addr as usize + 1 > AVAILABLE_STORAGE {
            return Err(Error::TooMuchData);
        }

        let addr = addr.to_be_bytes();

        self.with_wp_low(|eeprom| {
            eeprom.i2c.write(eeprom.address, &[addr[0], addr[1], byte])
                .map_err(Error::I2cWrite)
        })?;

        Ok(())
    }

    pub fn write_data(
        &mut self,
        addr: u16,
        data: &[u8],
    ) -> Result<(), Error<I2C>> {
        if addr as usize > AVAILABLE_STORAGE {
            return Err(Error::OutOfRange);
        }

        let len = data.len();
        if addr as usize + len > AVAILABLE_STORAGE {
            return Err(Error::TooMuchData);
        }

        let mut addr: u16 = addr;
        let mut writebuf: [u8; PAGESIZE + 2] = [0; PAGESIZE + 2];
        let mut wrptr: usize = 0;
        while wrptr < data.len() {
            let index: usize = addr as usize;
            let maxsize: usize = PAGESIZE - (index % PAGESIZE);
            let pagesize = if (len - wrptr) < maxsize {
                len - wrptr
            } else {
                maxsize
            };

            writebuf[0..2].clone_from_slice(&addr.to_be_bytes());
            writebuf[2..2 + pagesize].clone_from_slice(&data[wrptr..wrptr + pagesize]);

            self.with_wp_low(|eeprom| {
                eeprom.i2c.write(eeprom.address, &writebuf[0..pagesize + 2])
                    .map_err(Error::I2cWrite)?;
                eeprom.ack_polling()
            })?;

            addr += pagesize as u16;
            wrptr += pagesize;
        }

        Ok(())
    }

    pub fn read_byte(&mut self, addr: u16) -> Result<u8, Error<I2C>> {
        if addr as usize > AVAILABLE_STORAGE {
            return Err(Error::OutOfRange);
        }

        if addr as usize + 1 > AVAILABLE_STORAGE {
            return Err(Error::TooMuchData);
        }
        let mut byte: [u8; 1] = [0];
        self.i2c.write_read(self.address, &addr.to_be_bytes(), &mut byte)
            .map_err(Error::I2cWriteRead)?;
        Ok(byte[0])
    }

    pub fn read_data(
        &mut self,
        addr: u16,
        data: &mut [u8],
    ) -> Result<(), Error<I2C>> {
        if addr as usize > AVAILABLE_STORAGE {
            return Err(Error::OutOfRange);
        }

        if addr as usize + data.len() > AVAILABLE_STORAGE {
            return Err(Error::TooMuchData);
        }
        self.i2c.write_read(self.address, &addr.to_be_bytes(), data)
            .map_err(Error::I2cWriteRead)?;
        Ok(())
    }
}
