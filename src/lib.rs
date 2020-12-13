#![no_std]

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

use core::convert::TryFrom;
use num_enum::TryFromPrimitive;

#[derive(Debug, Eq, PartialEq)]
pub enum SMBCommandType {
    QuickCommandRead,
    QuickCommandWrite,
    ReadByte,
    SendByte,
    WriteByteData,
    WriteWordData,
    WriteBlockData,
    ReadByteData,
    ReadWordData,
    ReadBlockData,
}

#[derive(Debug, Default)]
struct State {
    byte_a: u8,
    byte_b: u8,
    byte_c: u8,
    some_byte: u8,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum WriteByteCommand {
    ResetByte = 0x0,
    IncrementByte = 0x1,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum ReadByteCommand {
    GetByte = 0x2,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum WriteByteDataCommand {
    SetByteA = 0x3,
    SetByteB = 0x4,
    SetByteC = 0x5,
}

#[repr(u8)]
#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum WriteWordDataCommand {
    SetByteAB = 0x6,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum ReadByteDataCommand {
    GetByteA = 0x7,
    GetByteB = 0x8,
    GetByteC = 0x9,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum ReadWordDataCommand {
    GetWordAB = 0xa,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum WriteBlockDataCommand {
    SetBlockABC = 0xb,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
pub enum ReadBlockDataCommand {
    GetBlockABC = 0xc,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Direction {
    MasterToSlave,
    SlaveToMaster,
}

pub enum I2CEvent<'a> {
    Addr { direction: Direction },
    ReceivedByte { byte: u8 },
    RequestedByte { byte: &'a mut u8 },
    Stop,
}

#[derive(Default, Debug)]
pub struct SMBusState {
    index: u8,
    received_data: [u8; 32],
    direction: Option<Direction>,
}

#[derive(Debug)]
pub enum SMBusProtoError {
    Unknown,
}

impl State {
    pub fn handle_i2c_event(
        &mut self,
        event: &mut I2CEvent,
        bus_state: &mut SMBusState,
    ) -> Result<(), SMBusProtoError> {
        match event {
            I2CEvent::Addr { direction } => bus_state.direction = Some(*direction),
            I2CEvent::ReceivedByte { byte } => {
                if bus_state.index == 0 {
                    bus_state.received_data[0] = *byte;
                } else {
                    if let Ok(command) = WriteByteDataCommand::try_from(bus_state.received_data[0])
                    {
                        match command {
                            WriteByteDataCommand::SetByteA => {
                                self.byte_a = *byte;
                            }
                            WriteByteDataCommand::SetByteB => {
                                self.byte_b = *byte;
                            }
                            WriteByteDataCommand::SetByteC => {
                                self.byte_c = *byte;
                            }
                        }
                    } else if let Ok(command) =
                        WriteWordDataCommand::try_from(bus_state.received_data[0])
                    {
                        match command {
                            WriteWordDataCommand::SetByteAB => match bus_state.index {
                                0 => self.byte_a = *byte,
                                1 => self.byte_b = *byte,
                                _ => return Err(SMBusProtoError::Unknown),
                            },
                        }
                    } else if let Ok(command) =
                        WriteBlockDataCommand::try_from(bus_state.received_data[0])
                    {
                        match command {
                            WriteBlockDataCommand::SetBlockABC => match bus_state.index {
                                1 => {
                                    if *byte != 3 {
                                        return Err(SMBusProtoError::Unknown);
                                    }
                                }
                                2 => self.byte_a = *byte,
                                3 => self.byte_b = *byte,
                                4 => self.byte_c = *byte,
                                _ => return Err(SMBusProtoError::Unknown),
                            },
                        }
                    }
                }
                bus_state.index += 1;
            }
            I2CEvent::RequestedByte { byte } => match bus_state.index {
                0 => {
                    if bus_state.direction == Some(Direction::SlaveToMaster) {
                        **byte = self.some_byte;
                    } else {
                        return Err(SMBusProtoError::Unknown);
                    }
                }
                _ => {
                    let first_byte = bus_state.received_data[0];
                    if let Ok(command) = ReadByteDataCommand::try_from(first_byte) {
                        match command {
                            ReadByteDataCommand::GetByteA => {
                                **byte = self.byte_a;
                            }
                            ReadByteDataCommand::GetByteB => {
                                **byte = self.byte_b;
                            }
                            ReadByteDataCommand::GetByteC => {
                                **byte = self.byte_c;
                            }
                        }
                    } else if let Ok(command) = ReadWordDataCommand::try_from(first_byte) {
                        match command {
                            ReadWordDataCommand::GetWordAB => {
                                if bus_state.index == 1 {
                                    **byte = self.byte_a;
                                } else if bus_state.index == 2 {
                                    **byte = self.byte_b;
                                }
                            }
                        }
                    } else if let Ok(command) = ReadBlockDataCommand::try_from(first_byte) {
                        match command {
                            ReadBlockDataCommand::GetBlockABC => match bus_state.index {
                                1 => **byte = 3,
                                2 => **byte = self.byte_a,
                                3 => **byte = self.byte_b,
                                4 => **byte = self.byte_c,
                                _ => return Err(SMBusProtoError::Unknown),
                            },
                        }
                    } else {
                        return Err(SMBusProtoError::Unknown);
                    }
                    bus_state.index += 1;
                }
            },
            I2CEvent::Stop => {
                if bus_state.index == 1 {
                    if let Ok(command) = WriteByteCommand::try_from(bus_state.received_data[0]) {
                        match command {
                            WriteByteCommand::ResetByte => {
                                self.some_byte = 0;
                            }
                            WriteByteCommand::IncrementByte => {
                                self.some_byte += 1;
                            }
                        }
                    }
                }
                bus_state.received_data.iter_mut().for_each(|x| *x = 0);
                bus_state.index = 0;
                bus_state.direction = None;
            }
        };
        return Ok(());
    }
}
