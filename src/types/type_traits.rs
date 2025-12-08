use ikm_calc::calculation::core::Constants;
use ikm_calc::calculation::kmh::KMHReport;

trait KMHReportExt {
  /// Обогащаем отчёт КМХ данными из Constants (прямой маппинг, без расчётов)
  fn apply_constants(&mut self, c: &Constants);

  /// Обновляем Constants на основе данных из KMHReport (обратный маппинг)
  fn write_to_constants(&self, c: &mut Constants);
}

impl KMHReportExt for KMHReport {
  /// Constants → KMHReport
  fn apply_constants(&mut self, c: &Constants) {
    // Масса понтона
    if let Some(pontoon) = c.pontoon_weight {
      self.pontoon_mass = pontoon;
    }

    // Коэф. линейного расширения стенки резервуара
    self.wall_alpha_coefficient = c.tank_wall_alpha;

    // Предел абсолютной погрешности измерения расстояния
    self.delta_height = c.max_level_abs_error;

    // Базовая/номинальная высота резервуара
    self.nominal_height = c.structure_base_height;

    // Плотность по ИС
    if let Some(rho) = c.tank_product_density {
      self.density_measured = rho;
    }
  }

  /// KMHReport → Constants
  fn write_to_constants(&self, c: &mut Constants) {
    // Масса понтона (в Constants — Option)
    c.pontoon_weight = Some(self.pontoon_mass);

    // Коэф. линейного расширения стенки резервуара
    c.tank_wall_alpha = self.wall_alpha_coefficient;

    // Предел абсолютной погрешности измерения расстояния
    c.max_level_abs_error = self.delta_height;

    // Базовая/номинальная высота резервуара
    c.structure_base_height = self.nominal_height;

    // Плотность по ИС (в Constants — Option)
    c.tank_product_density = Some(self.density_measured);
  }
}