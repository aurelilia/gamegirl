use eframe::egui::Ui;
use gamegirl::system::cpu::DReg;
use gamegirl::system::GameGirl;

pub fn registers(gg: &GameGirl, ui: &mut Ui) {
    for reg in [DReg::AF, DReg::BC, DReg::DE, DReg::HL] {
        ui.label(format!("{:?} = {:04X}", reg, gg.cpu.dreg(reg)));
    }
}
