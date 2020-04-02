use crate::device::{Port, PortIoValue};
use crate::error::{Error, Result};
use crate::memory;
use crate::{vcpu, vmcs, vmexit};
use core::convert::TryFrom;

fn emulate_outs(
    vcpu: &mut vcpu::VCpu,
    port: Port,
    guest_cpu: &mut vmexit::GuestCpuState,
    exit: vmexit::IoInstructionInformation,
) -> Result<()> {
    let mut vm = vcpu.vm.write();

    let linear_addr =
        vcpu.vmcs.read_field(vmcs::VmcsField::GuestLinearAddress)?;
    let guest_addr = memory::GuestVirtAddr::new(linear_addr, &vcpu.vmcs)?;

    // FIXME: This could actually be any priv level due to IOPL, but for now
    //        assume that is requires supervisor
    let access = memory::GuestAccess::Read(memory::PrivilegeLevel(0));

    // FIXME: The direction we read is determined by the DF flag (I think)
    // FIXME: We should probably only be using some of the lower order bits
    let bytes = vm.guest_space.read_bytes(
        &vcpu.vmcs,
        guest_addr,
        (guest_cpu.rcx * exit.size as u64) as usize,
        access,
    )?;

    let dev = vm.config.device_map().device_for_mut(port).ok_or_else(|| {
        Error::MissingDevice(format!("No device for port {}", port))
    })?;

    // FIXME: Actually test for REP
    for chunk in bytes.chunks_exact(exit.size as usize) {
        dev.on_port_write(port, PortIoValue::try_from(chunk)?)?;
    }

    guest_cpu.rsi += bytes.len() as u64;
    guest_cpu.rcx = 0;
    Ok(())
}

fn emulate_ins(
    vcpu: &mut vcpu::VCpu,
    port: Port,
    guest_cpu: &mut vmexit::GuestCpuState,
    exit: vmexit::IoInstructionInformation,
) -> Result<()> {
    let mut vm = vcpu.vm.write();

    let dev = vm.config.device_map().device_for_mut(port).ok_or_else(|| {
        Error::MissingDevice(format!("No device for port {}", port))
    })?;

    let linear_addr =
        vcpu.vmcs.read_field(vmcs::VmcsField::GuestLinearAddress)?;
    let guest_addr = memory::GuestVirtAddr::new(linear_addr, &vcpu.vmcs)?;
    let access = memory::GuestAccess::Read(memory::PrivilegeLevel(0));

    let mut bytes = vec![0u8; guest_cpu.rcx as usize];
    for chunk in bytes.chunks_exact_mut(exit.size as usize) {
        let mut val = PortIoValue::try_from(&*chunk)?;
        dev.on_port_read(port, &mut val)?;
        chunk.copy_from_slice(val.as_slice());
    }

    vm.guest_space
        .write_bytes(&vcpu.vmcs, guest_addr, &bytes, access)?;

    guest_cpu.rdi += bytes.len() as u64;
    guest_cpu.rcx = 0;
    Ok(())
}

pub fn emulate_portio(
    vcpu: &mut vcpu::VCpu,
    guest_cpu: &mut vmexit::GuestCpuState,
    exit: vmexit::IoInstructionInformation,
) -> Result<()> {
    let (port, input, size, string) =
        (exit.port, exit.input, exit.size, exit.string);

    if !string {
        let mut vm = vcpu.vm.write();

        let dev =
            vm.config.device_map().device_for_mut(port).ok_or_else(|| {
                Error::MissingDevice(format!("No device for port {}", port))
            })?;

        if !input {
            let arr = (guest_cpu.rax as u32).to_be_bytes();
            dev.on_port_write(
                port,
                PortIoValue::try_from(&arr[4 - size as usize..])?,
            )?;
        } else {
            let mut val = match size {
                1 => PortIoValue::OneByte([0]),
                2 => PortIoValue::TwoBytes([0, 0]),
                4 => PortIoValue::FourBytes([0, 0, 0, 0]),
                _ => panic!("Invalid portio read size: {}", size),
            };
            dev.on_port_read(port, &mut val)?;
            guest_cpu.rax &= (!guest_cpu.rax) << (size * 8);
            guest_cpu.rax |= val.as_u32() as u64;
        }
    } else {
        if !input {
            emulate_outs(vcpu, port, guest_cpu, exit)?;
        } else {
            emulate_ins(vcpu, port, guest_cpu, exit)?;
        }
    }
    Ok(())
}