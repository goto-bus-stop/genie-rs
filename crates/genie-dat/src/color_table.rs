use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::io::{Read, Result, Write};

/// Player colour data.
#[derive(Debug, Clone)]
pub struct ColorTable {
    id: i32,
    /// Base palette index for this player colour.
    base: i32,
    /// The palette index to use for unit outlines when they are obscured by buildings or trees.
    unit_outline_color: i32,
    unit_selection_colors: (i32, i32),
    /// Palette indices for this colour on the minimap.
    minimap_colors: (i32, i32, i32),
    /// Palette index to use for this player colour in the in-game statistics in the bottom right.
    statistics_text_color: i32,
}

impl ColorTable {
    pub fn from<R: Read>(input: &mut R) -> Result<Self> {
        let id = input.read_i32::<LE>()?;
        let base = input.read_i32::<LE>()?;
        let unit_outline_color = input.read_i32::<LE>()?;
        let unit_selection_colors = (input.read_i32::<LE>()?, input.read_i32::<LE>()?);
        let minimap_colors = (
            input.read_i32::<LE>()?,
            input.read_i32::<LE>()?,
            input.read_i32::<LE>()?,
        );
        let statistics_text_color = input.read_i32::<LE>()?;

        Ok(Self {
            id,
            base,
            unit_outline_color,
            unit_selection_colors,
            minimap_colors,
            statistics_text_color,
        })
    }

    pub fn write_to<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_i32::<LE>(self.id)?;
        output.write_i32::<LE>(self.base)?;
        output.write_i32::<LE>(self.unit_outline_color)?;
        output.write_i32::<LE>(self.unit_selection_colors.0)?;
        output.write_i32::<LE>(self.unit_selection_colors.1)?;
        output.write_i32::<LE>(self.minimap_colors.0)?;
        output.write_i32::<LE>(self.minimap_colors.1)?;
        output.write_i32::<LE>(self.minimap_colors.2)?;
        output.write_i32::<LE>(self.statistics_text_color)?;
        Ok(())
    }
}