use bytemuck::from_bytes;

use crate::elf::CodeSegment;
use crate::{
    chip::Esp32Params,
    elf::{FirmwareImage, RomSegment},
    error::Error,
    image_format::{EspCommonHeader, ImageFormat, ESP_MAGIC},
};
use std::io::Write;
use std::{borrow::Cow, iter::once};

/// Image format for esp32 family chips using a 2nd stage bootloader
pub struct Esp32McuBootFormat<'a> {
    params: Esp32Params,
    bootloader: Cow<'a, [u8]>,
    flash_segment: RomSegment<'a>,
}

impl<'a> Esp32McuBootFormat<'a> {
    pub fn new(image: &'a FirmwareImage,
        params: Esp32Params,
        bootloader: Option<Vec<u8>>
    ) -> Result<Self, Error> {
        let bootloader = if let Some(bytes) = bootloader {
            Cow::Owned(bytes)
        } else {
            Cow::Borrowed(params.default_mcuboot.unwrap())
        };

        let mut data = Vec::new();

        // fetch the generated header from the bootloader
        let header: EspCommonHeader = *from_bytes(&bootloader[0..8]);
        if header.magic != ESP_MAGIC {
            return Err(Error::InvalidBootloader);
        }

        let mut segment = image
            .segments_with_load_addresses()
            .fold(CodeSegment::default(), |mut a, b| {
                a += &b;
                a
            });
        segment.pad_align(4);

        if segment.addr != 0
            || segment.data()[32..36] != [0xd3, 0x37, 0xe6, 0xac]
        {
            return Err(Error::InvalidMcuBootBinary);
        }

        data.write_all(segment.data())?;

        let flash_segment = RomSegment {
            addr: params.app_addr,
            data: Cow::Owned(data),
        };

        Ok(Self {
            params,
            bootloader,
            flash_segment,
        })
    }
}

impl<'a> ImageFormat<'a> for Esp32McuBootFormat<'a> {
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(
            once(RomSegment {
                addr: self.params.boot_addr,
                data: Cow::Borrowed(&self.bootloader),
            })
            .chain(once(self.flash_segment.borrow())),
        )
    }

    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(once(self.flash_segment.borrow()))
    }
}
