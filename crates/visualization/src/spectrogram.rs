use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat, VertexFormat,
};
use bevy::shader::ShaderRef;
use glowfish_data::{Grid, MinMaxCell, Scaling, ValueRange};

use crate::layout::{FREQ_EXTENT, TIME_EXTENT, VALUE_EXTENT};
use crate::{
    CellValue, DataKind, Dataset, DisplayMode, DisplayedGrid, Selection, VisualizationConfig,
    VisualizationEntity,
};

const SHADER_PATH: &str = "embedded://visualization/shaders/spectrogram.wesl";

/// Index of the cell (time, frequency) a vertex belongs to.
pub const ATTRIBUTE_CELL: MeshVertexAttribute =
    MeshVertexAttribute::new("SpectrogramCell", 2_903_517_441, VertexFormat::Uint32x2);

/// Gap between neighbouring cubes, as a fraction of a cell.
const CUBE_GAP: f32 = 0.06;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SpectrogramMaterial {
    #[uniform(0)]
    pub params: SpectrogramParams,
    /// `Rg32Float` texture with (min, max) per cell; x is time, y is frequency.
    #[texture(1, sample_type = "float", filterable = false)]
    pub data: Handle<Image>,
}

#[derive(ShaderType, Debug, Clone, Copy, Default, PartialEq)]
pub struct SpectrogramParams {
    pub dims: UVec2,
    pub value_min: f32,
    pub value_max: f32,
    pub freq_extent: f32,
    pub mode: u32,
    pub cell_value: u32,
    pub clip_x: Vec2,
}

impl Material for SpectrogramMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            ATTRIBUTE_CELL.at_shader_location(2),
        ])?];
        // surfaces are seen from below as well
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The entity drawing the spectrogram.
#[derive(Component)]
pub struct SpectrogramView;

/// Meshes for the current cell budget; built when first needed.
#[derive(Resource, Default)]
struct SpectrogramMeshes {
    dims: (usize, usize),
    cubes: Option<Handle<Mesh>>,
    surface: Option<Handle<Mesh>>,
}

pub struct SpectrogramPlugin;

impl Plugin for SpectrogramPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/spectrogram.wesl");
        app.add_plugins(MaterialPlugin::<SpectrogramMaterial>::default())
            .init_resource::<SpectrogramMeshes>()
            .add_systems(
                Update,
                (
                    spawn_spectrogram.run_if(resource_changed::<Dataset>),
                    update_mesh,
                    update_data,
                    update_transform,
                )
                    .chain()
                    .after(crate::DataSystems)
                    .run_if(resource_exists::<Dataset>),
            );
    }
}

fn spawn_spectrogram(
    mut commands: Commands,
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SpectrogramMaterial>>,
) {
    if dataset.kind != DataKind::Spectrogram {
        return;
    }
    let (time, freq) = texture_size(&dataset, &config);
    let data = images.add(data_image(time, freq));
    commands.spawn((
        SpectrogramView,
        VisualizationEntity,
        Mesh3d::default(),
        MeshMaterial3d(materials.add(SpectrogramMaterial {
            params: SpectrogramParams::default(),
            data,
        })),
        Transform::default(),
    ));
}

/// Texture (and mesh) size: the cell budget plus one time cell for alignment.
fn texture_size(dataset: &Dataset, config: &VisualizationConfig) -> (usize, usize) {
    let (time, freq) = crate::grid_size(dataset.kind, config.max_cells, dataset.bands());
    (time + 1, freq)
}

fn data_image(time: usize, freq: usize) -> Image {
    Image::new(
        Extent3d {
            width: time as u32,
            height: freq as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; time * freq * 8],
        TextureFormat::Rg32Float,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

/// Swaps the mesh when the display mode or the cell budget changes.
fn update_mesh(
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    mut cache: ResMut<SpectrogramMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut views: Query<&mut Mesh3d, With<SpectrogramView>>,
    added: Query<(), Added<SpectrogramView>>,
) {
    if !config.is_changed() && added.is_empty() {
        return;
    }
    let dims = texture_size(&dataset, &config);
    if cache.dims != dims {
        *cache = SpectrogramMeshes { dims, ..default() };
    }
    let handle = match config.mode {
        DisplayMode::Cubes => cache
            .cubes
            .get_or_insert_with(|| meshes.add(cubes_mesh(dims.0, dims.1)))
            .clone(),
        DisplayMode::Surface => cache
            .surface
            .get_or_insert_with(|| meshes.add(surface_mesh(dims.0, dims.1)))
            .clone(),
    };
    for mut mesh in &mut views {
        if mesh.0 != handle {
            mesh.0 = handle.clone();
        }
    }
}

/// Uploads a new grid into the data texture.
fn update_data(
    dataset: Res<Dataset>,
    config: Res<VisualizationConfig>,
    displayed: Option<Res<DisplayedGrid>>,
    views: Query<&MeshMaterial3d<SpectrogramMaterial>, With<SpectrogramView>>,
    added: Query<(), Added<SpectrogramView>>,
    mut materials: ResMut<Assets<SpectrogramMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(displayed) = displayed else { return };
    if !displayed.is_changed() && !config.is_changed() && added.is_empty() {
        return;
    }
    let grid = &displayed.grid;
    for material in &views {
        let Some(mut material) = materials.get_mut(&material.0) else {
            continue;
        };
        let range = value_range(config.scaling, dataset.full_scale, grid);
        material.params = SpectrogramParams {
            dims: UVec2::new(grid.time_cells as u32, grid.freq_cells as u32),
            value_min: range.min,
            value_max: range.max,
            freq_extent: grid.bands as f32 / grid.bands_per_cell as f32,
            mode: match config.mode {
                DisplayMode::Cubes => 0,
                DisplayMode::Surface => 1,
            },
            cell_value: match config.cell_value {
                CellValue::Max => 0,
                CellValue::Range => 1,
            },
            clip_x: material.params.clip_x,
        };
        if let Some(mut image) = images.get_mut(&material.data) {
            let width = image.width() as usize;
            if let Some(bytes) = image.data.as_mut() {
                write_cells(bytes, width, grid);
            }
        }
    }
}

/// The displayed value range for `scaling`.
pub fn value_range(scaling: Scaling, full_scale: f32, grid: &Grid<MinMaxCell>) -> ValueRange {
    scaling.resolve(full_scale, grid.value_range())
}

/// Writes the (min, max) of every grid cell into `Rg32Float` texture bytes of `width` texels.
fn write_cells(bytes: &mut [u8], width: usize, grid: &Grid<MinMaxCell>) {
    let time_cells = grid.time_cells.min(width);
    for f in 0..grid.freq_cells {
        for t in 0..time_cells {
            let cell = grid.cells[t * grid.freq_cells + f];
            let at = (f * width + t) * 8;
            if let Some(texel) = bytes.get_mut(at..at + 8) {
                texel[..4].copy_from_slice(&cell.min.to_le_bytes());
                texel[4..].copy_from_slice(&cell.max.to_le_bytes());
            }
        }
    }
}

/// Places the mesh (in cell units) so that the selection fills the time extent; runs every time
/// the selection moves, so panning is immediate even before new data arrived.
fn update_transform(
    selection: Option<Res<Selection>>,
    displayed: Option<Res<DisplayedGrid>>,
    mut views: Query<(&mut Transform, &MeshMaterial3d<SpectrogramMaterial>), With<SpectrogramView>>,
    mut materials: ResMut<Assets<SpectrogramMaterial>>,
) {
    let (Some(selection), Some(displayed)) = (selection, displayed) else {
        return;
    };
    if !selection.is_changed() && !displayed.is_changed() {
        return;
    }
    let placement = Placement::new(&displayed.grid, &selection);
    for (mut transform, material) in &mut views {
        *transform = placement.transform;
        if let Some(mut material) = materials.get_mut(&material.0)
            && material.params.clip_x != placement.clip_x
        {
            material.params.clip_x = placement.clip_x;
        }
    }
}

/// Where the cells of a grid go in the world for a selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Maps cell units (x: time cell, z: frequency cell, y: normalized value) to the world.
    pub transform: Transform,
    /// The local x range inside the selection.
    pub clip_x: Vec2,
}

impl Placement {
    pub fn new(grid: &Grid<MinMaxCell>, selection: &Selection) -> Self {
        let span = selection.span().max(f64::MIN_POSITIVE);
        let rows_per_cell = grid.rows_per_cell() as f64;
        let first_row = grid.first_cell as f64 * rows_per_cell;
        let scale_x = rows_per_cell / span * TIME_EXTENT as f64;
        let offset_x = (first_row - selection.start) / span * TIME_EXTENT as f64;
        let freq_extent = grid.bands as f32 / grid.bands_per_cell as f32;
        let transform = Transform::from_xyz(offset_x as f32, 0.0, 0.0).with_scale(Vec3::new(
            scale_x as f32,
            VALUE_EXTENT,
            FREQ_EXTENT / freq_extent,
        ));
        let to_local = |world: f64| ((world - offset_x) / scale_x) as f32;
        Self {
            transform,
            clip_x: Vec2::new(to_local(0.0), to_local(TIME_EXTENT as f64)),
        }
    }
}

/// Unit cubes on a `time` × `freq` cell grid: a top and four sides per cell (the bottom is never
/// visible). Heights are set by the shader.
pub fn cubes_mesh(time: usize, freq: usize) -> Mesh {
    let cells = time * freq;
    let mut positions = Vec::with_capacity(cells * 20);
    let mut normals = Vec::with_capacity(cells * 20);
    let mut cell_ids = Vec::with_capacity(cells * 20);
    let mut indices = Vec::with_capacity(cells * 30);
    for t in 0..time {
        for f in 0..freq {
            let (x0, x1) = (t as f32 + CUBE_GAP, (t + 1) as f32 - CUBE_GAP);
            let (z0, z1) = (f as f32 + CUBE_GAP, (f + 1) as f32 - CUBE_GAP);
            let faces: [([[f32; 3]; 4], [f32; 3]); 5] = [
                (
                    [[x0, 1., z0], [x1, 1., z0], [x1, 1., z1], [x0, 1., z1]],
                    [0., 1., 0.],
                ),
                (
                    [[x0, 0., z0], [x1, 0., z0], [x1, 1., z0], [x0, 1., z0]],
                    [0., 0., -1.],
                ),
                (
                    [[x0, 0., z1], [x1, 0., z1], [x1, 1., z1], [x0, 1., z1]],
                    [0., 0., 1.],
                ),
                (
                    [[x0, 0., z0], [x0, 0., z1], [x0, 1., z1], [x0, 1., z0]],
                    [-1., 0., 0.],
                ),
                (
                    [[x1, 0., z0], [x1, 0., z1], [x1, 1., z1], [x1, 1., z0]],
                    [1., 0., 0.],
                ),
            ];
            for (corners, normal) in faces {
                let base = positions.len() as u32;
                positions.extend(corners);
                normals.extend([normal; 4]);
                cell_ids.extend([[t as u32, f as u32]; 4]);
                indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    build_mesh(positions, normals, cell_ids, indices)
}

/// A height field with one vertex per cell centre; heights and normals are set by the shader.
pub fn surface_mesh(time: usize, freq: usize) -> Mesh {
    let mut positions = Vec::with_capacity(time * freq);
    let mut cell_ids = Vec::with_capacity(time * freq);
    for t in 0..time {
        for f in 0..freq {
            positions.push([t as f32 + 0.5, 1.0, f as f32 + 0.5]);
            cell_ids.push([t as u32, f as u32]);
        }
    }
    let vertex = |t: usize, f: usize| (t * freq + f) as u32;
    let mut indices = Vec::with_capacity(time.saturating_sub(1) * freq.saturating_sub(1) * 6);
    for t in 0..time.saturating_sub(1) {
        for f in 0..freq.saturating_sub(1) {
            let (a, b) = (vertex(t, f), vertex(t + 1, f));
            let (c, d) = (vertex(t + 1, f + 1), vertex(t, f + 1));
            indices.extend([a, b, c, a, c, d]);
        }
    }
    let normals = vec![[0., 1., 0.]; positions.len()];
    build_mesh(positions, normals, cell_ids, indices)
}

fn build_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    cell_ids: Vec<[u32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(ATTRIBUTE_CELL, cell_ids)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(first_cell: u64, level: u32, time_cells: usize) -> Grid<MinMaxCell> {
        Grid {
            range: 0..1,
            level,
            first_cell,
            time_cells,
            freq_cells: 3,
            bands_per_cell: 4,
            bands: 10,
            cells: vec![MinMaxCell { min: 0.0, max: 1.0 }; time_cells * 3],
        }
    }

    #[test]
    fn mesh_sizes() {
        let cubes = cubes_mesh(4, 3);
        assert_eq!(cubes.count_vertices(), 4 * 3 * 20);
        assert_eq!(cubes.indices().unwrap().len(), 4 * 3 * 30);
        let surface = surface_mesh(4, 3);
        assert_eq!(surface.count_vertices(), 12);
        assert_eq!(surface.indices().unwrap().len(), 3 * 2 * 6);
        assert_eq!(surface_mesh(1, 5).indices().unwrap().len(), 0);
    }

    #[test]
    fn placement_maps_the_selection_to_the_time_extent() {
        // cells of 4 rows starting at row 8, selection 10..30
        let placement = Placement::new(&grid(2, 2, 6), &Selection::new(10.0, 30.0));
        let world_x = |local: f32| placement.transform.transform_point(Vec3::X * local).x;
        // local x is in cells: cell 0 starts at row 8 -> world (8 - 10) / 20 * 16
        assert!((world_x(0.0) - (-1.6)).abs() < 1e-5);
        // row 30 = cell 5.5
        assert!((world_x(5.5) - TIME_EXTENT).abs() < 1e-4);
        assert!((placement.clip_x.x - 0.5).abs() < 1e-5);
        assert!((placement.clip_x.y - 5.5).abs() < 1e-5);
        // 10 bands in cells of 4 -> 2.5 cells fill the frequency extent
        let world_z = placement.transform.transform_point(Vec3::Z * 2.5).z;
        assert!((world_z - FREQ_EXTENT).abs() < 1e-5);
    }

    #[test]
    fn writes_cells_row_by_frequency() {
        let mut g = grid(0, 0, 2);
        g.cells[1] = MinMaxCell { min: 5.0, max: 6.0 }; // t = 0, f = 1
        let mut bytes = vec![0; 3 * 3 * 8];
        write_cells(&mut bytes, 3, &g);
        let at = 3 * 8; // f = 1, t = 0
        assert_eq!(
            f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()),
            5.0
        );
        assert_eq!(
            f32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()),
            6.0
        );
    }
}
