//! Composite force field combining multiple components

use crate::types::{Atom, SimulationBox};
use crate::neighbor::NeighborList;
use super::traits::ForceField;

/// A composite force field that combines multiple force field components
#[derive(Default)]
pub struct CompositeForceField {
    /// Force field components
    components: Vec<Box<dyn ForceField>>,
    /// Maximum cutoff among all components
    max_cutoff: f32,
}

impl CompositeForceField {
    /// Create a new empty composite force field
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            max_cutoff: 0.0,
        }
    }

    /// Add a force field component
    pub fn add<F: ForceField + 'static>(mut self, ff: F) -> Self {
        let cutoff = ff.cutoff();
        if cutoff > self.max_cutoff {
            self.max_cutoff = cutoff;
        }
        self.components.push(Box::new(ff));
        self
    }

    /// Add a boxed force field component
    pub fn add_boxed(mut self, ff: Box<dyn ForceField>) -> Self {
        let cutoff = ff.cutoff();
        if cutoff > self.max_cutoff {
            self.max_cutoff = cutoff;
        }
        self.components.push(ff);
        self
    }

    /// Get the number of components
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Get component names
    pub fn component_names(&self) -> Vec<&str> {
        self.components.iter().map(|c| c.name()).collect()
    }
}

impl ForceField for CompositeForceField {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) {
        for component in &self.components {
            component.compute_forces(atoms, box_, neighbor_list);
        }
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) -> f64 {
        self.components
            .iter()
            .map(|c| c.potential_energy(atoms, box_, neighbor_list))
            .sum()
    }

    fn cutoff(&self) -> f32 {
        self.max_cutoff
    }

    fn name(&self) -> &str {
        "Composite"
    }

    fn requires_neighbor_list(&self) -> bool {
        self.components.iter().any(|c| c.requires_neighbor_list())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::force_field::LennardJones;

    #[test]
    fn test_composite_creation() {
        let lj = LennardJones::argon();
        let composite = CompositeForceField::new().add(lj);

        assert_eq!(composite.len(), 1);
        assert!((composite.cutoff() - 2.5).abs() < 1e-6);
    }
}
