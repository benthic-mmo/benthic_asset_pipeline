use benthic_protocol::default_animations::{DefaultAnimation, JointAnimation, Keyframe};
use benthic_protocol::skeleton::{Joint, JointName, Skeleton, Transform};
use bvh_anim::ChannelType;
use glam::Mat4;
use glam::{Quat, Vec3};
use gltf;
use gltf::Node;
use indexmap::IndexMap;
use std::io::BufReader;
use std::{collections::HashMap, env, fs, path::PathBuf, str::FromStr};
use std::{fs::File, io::Write};
use uuid::Uuid;

fn main() {
    let gen_animations = std::env::var("CARGO_FEATURE_ANIMATIONS").is_ok();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let base_skeleton_path = benthic_default_assets::skeleton().join("skeleton.gltf");
    // TODO: stop generating code. just export this as json and write it to disk.
    let skeleton = skeleton_from_gltf(base_skeleton_path);
    let skeleton_code = generate_skeleton_code(&skeleton);

    let skeleton_file = out_dir.join("default_skeleton.rs");
    let mut file = File::create(&skeleton_file).unwrap();
    write!(
        file,
        "pub static DEFAULT_SKELETON: once_cell::sync::Lazy<benthic_protocol::skeleton::Skeleton> = once_cell::sync::Lazy::new(|| {});\n",
        skeleton_code
    )
    .unwrap();
    println!("cargo:warning=Generating {:?}", skeleton_file);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    if gen_animations {
        let animation_path = benthic_default_assets::animations();

        for entry in fs::read_dir(&animation_path).unwrap() {
            let path = entry.unwrap().path();

            let animation_name =
                DefaultAnimation::from_str(&path.file_stem().unwrap().to_string_lossy()).unwrap();

            let extension = path.extension().and_then(|e| e.to_str());

            let animation_data = match extension {
                Some("gltf") | Some("glb") => load_gltf_animation(&path),
                Some("bvh") => load_bvh_animation(&path),
                _ => continue,
            };

            let file_name = format!("{:?}.json", animation_name);
            let out_path = out_dir.join("Animations");
            fs::create_dir_all(&out_path).unwrap();
            let out_path = out_path.join(file_name);

            let json = serde_json::to_string_pretty(&animation_data)
                .expect("Failed to serialize animation");
            fs::write(&out_path, json).unwrap();

            println!("cargo:warning=Generating {:?}", out_path);
        }
    }
}

fn load_gltf_animation(path: &PathBuf) -> Vec<JointAnimation> {
    let (document, buffers, _) = gltf::import(path).unwrap();

    let mut animation_data: HashMap<JointName, JointAnimation> = HashMap::new();

    for anim in document.animations() {
        for channel in anim.channels() {
            let target = channel.target();

            let joint_name = JointName::from_str(target.node().name().unwrap()).unwrap();

            let property = target.property();

            let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(|d| &d.0[..]));

            let times: Vec<f32> = reader.read_inputs().unwrap().collect();

            let entry = animation_data
                .entry(joint_name)
                .or_insert_with(|| JointAnimation {
                    joint: joint_name,
                    translations: Vec::new(),
                    rotations: Vec::new(),
                    scales: Vec::new(),
                });

            match property {
                gltf::animation::Property::Translation => {
                    if let Some(gltf::animation::util::ReadOutputs::Translations(t)) =
                        reader.read_outputs()
                    {
                        entry
                            .translations
                            .extend(times.iter().zip(t).map(|(time, value)| Keyframe {
                                time: *time,
                                value: Vec3::from_slice(&value),
                            }));
                    }
                }

                gltf::animation::Property::Rotation => {
                    if let Some(gltf::animation::util::ReadOutputs::Rotations(r)) =
                        reader.read_outputs()
                    {
                        entry.rotations.extend(times.iter().zip(r.into_f32()).map(
                            |(time, value)| Keyframe {
                                time: *time,
                                value: Quat::from_array(value),
                            },
                        ));
                    }
                }

                gltf::animation::Property::Scale => {
                    if let Some(gltf::animation::util::ReadOutputs::Scales(s)) =
                        reader.read_outputs()
                    {
                        entry
                            .scales
                            .extend(times.iter().zip(s).map(|(time, value)| Keyframe {
                                time: *time,
                                value: Vec3::from_slice(&value),
                            }));
                    }
                }

                _ => {}
            }
        }
    }

    animation_data.into_values().collect()
}

fn load_bvh_animation(path: &PathBuf) -> Vec<JointAnimation> {
    let bvh_file = File::open(path).unwrap();

    let bvh = bvh_anim::from_reader(BufReader::new(bvh_file)).unwrap();

    let mut animation_data: HashMap<JointName, JointAnimation> = HashMap::new();

    let axis_correction = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);

    for (frame_idx, frame) in bvh.frames().enumerate() {
        let frame_time = bvh.frame_time().as_secs_f32();

        let time = frame_idx as f32 * frame_time;

        for joint in bvh.joints() {
            let joint_name =
                match JointName::from_str_non_bento(joint.data().name().to_str().unwrap()) {
                    Some(j) => j,
                    None => continue,
                };

            let mut translation = Vec3::ZERO;
            let mut rotation = Quat::IDENTITY;

            for channel in joint.data().channels() {
                let value = frame.get(channel).unwrap();

                let radians = value.to_radians();

                match channel.channel_type() {
                    ChannelType::PositionX => translation.x = *value,

                    ChannelType::PositionY => translation.y = *value,

                    ChannelType::PositionZ => translation.z = *value,

                    ChannelType::RotationX => rotation = rotation * Quat::from_rotation_x(radians),

                    ChannelType::RotationY => rotation = rotation * Quat::from_rotation_y(radians),

                    ChannelType::RotationZ => rotation = rotation * Quat::from_rotation_z(radians),
                }
            }

            let corrected_translation = (axis_correction * translation) * 0.01;

            let corrected_rotation = match joint_name {
                JointName::Pelvis => rotation,

                _ => axis_correction * rotation * axis_correction.inverse(),
            };

            let entry = animation_data
                .entry(joint_name)
                .or_insert_with(|| JointAnimation {
                    joint: joint_name,
                    translations: Vec::new(),
                    rotations: Vec::new(),
                    scales: Vec::new(),
                });

            if joint_name == JointName::Pelvis {
                entry.translations.push(Keyframe {
                    time,
                    value: corrected_translation,
                });
            }

            entry.rotations.push(Keyframe {
                time,
                value: corrected_rotation,
            });

            entry.scales.push(Keyframe {
                time,
                value: Vec3::ONE,
            });
        }
    }

    animation_data.into_values().collect()
}

/// This is used to generate the default keleton from the GLTF file. This allows for creating
/// skeletons with default transforms without having to reread the file every time an avatar loads
/// in.
fn skeleton_from_gltf(skeleton_path: PathBuf) -> Skeleton {
    let (document, buffers, _) = gltf::import(&skeleton_path)
        .unwrap_or_else(|_| panic!("Failed to load skeleton {:?}", skeleton_path));
    let skin = document.skins().next().expect("No skins in gltf");

    let nodes: Vec<Node> = document.nodes().collect();
    let ibm_accessor = skin
        .inverse_bind_matrices()
        .expect("Skin has no inverse bind matrices");

    let view = ibm_accessor.view().expect("Accessor has no buffer view");
    let buffer_data = &buffers[view.buffer().index()];
    let ibm_offset = ibm_accessor.offset() + view.offset();
    let ibm_stride = view.stride().unwrap_or(16 * 4); // 16 floats * 4 bytes
    let ibm_count = ibm_accessor.count();

    // Map node index to IBM
    // TODO: This should be moved to build_joint_recursive
    let mut ibm_map: HashMap<usize, Mat4> = HashMap::new();
    for (i, node) in skin.joints().enumerate() {
        if i >= ibm_count {
            panic!(
                "Joint index {} out of bounds for IBMs count {}",
                i, ibm_count
            );
        }

        let start = ibm_offset + i * ibm_stride;
        let end = start + 16 * 4;
        let matrix_bytes = &buffer_data[start..end];
        let matrix_floats: &[f32] = bytemuck::cast_slice(matrix_bytes);
        let matrix_floats: &[f32; 16] = matrix_floats
            .try_into()
            .expect("Invalid matrix slice length");
        ibm_map.insert(node.index(), Mat4::from_cols_array(matrix_floats));
    }

    let mut joints = IndexMap::new();
    // 158 is the index of mpelvis
    build_joint_recursive(158, None, 0, &nodes, &mut joints, &ibm_map);
    Skeleton {
        joints,
        root: vec![JointName::Pelvis],
    }
}

fn build_joint_recursive(
    index: usize,
    parent: Option<JointName>,
    parent_index: usize,
    nodes: &[Node],
    joints: &mut IndexMap<JointName, Joint>,
    ibm_map: &HashMap<usize, Mat4>,
) {
    let node = nodes[index].clone();
    let name = JointName::from_str(node.name().unwrap()).unwrap();
    if joints.contains_key(&name) {
        return;
    }

    let mut children = Vec::new();
    for child in node.children() {
        children.push(
            JointName::from_str(child.name().unwrap())
                .unwrap_or_else(|err| panic!("errored on {:?}, {:?}", child.name(), err)),
        );
        build_joint_recursive(
            child.index(),
            Some(name),
            node.index(),
            nodes,
            joints,
            ibm_map,
        );
    }
    let global = ibm_map[&index];
    let local = if index == parent_index {
        global
    } else {
        ibm_map[&parent_index] * global.inverse()
    };
    let joint = Joint {
        name,
        parent,
        children,
        transforms: vec![Transform {
            name: "Default".to_string(),
            id: Uuid::nil(),
            transform: global,
            rank: 0,
        }],
        local_transforms: vec![Transform {
            name: "Default".to_string(),
            id: Uuid::nil(),
            transform: local,
            rank: 0,
        }],
    };
    joints.insert(name, joint);
}

fn generate_skeleton_code(skeleton: &Skeleton) -> String {
    let joints_code = skeleton
        .joints
        .iter()
        .map(|(name, joint)| {
            let children = joint
                .children
                .iter()
                .map(|c| format!("benthic_protocol::skeleton::JointName::{:?}", c))
                .collect::<Vec<_>>()
                .join(", ");

            let transform = joint.transforms[0].transform.to_cols_array();
            let transform_str = format!(
                "glam::Mat4::from_cols_array(&[{}])",
                transform
                    .iter()
                    .map(|f| format!("{:?}", f))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let local_transform = joint.local_transforms[0].transform.to_cols_array();
            let local_transform_str = format!(
                "glam::Mat4::from_cols_array(&[{}])",
                local_transform
                    .iter()
                    .map(|f| format!("{:?}", f))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            format!(
                "(benthic_protocol::skeleton::JointName::{n}, benthic_protocol::skeleton::Joint {{
                name: benthic_protocol::skeleton::JointName::{n},
                parent: {parent},
                children: vec![{children}],
                transforms: vec![
                    benthic_protocol::skeleton::Transform{{
                        name:\"Default\".to_string(), 
                        id: uuid::Uuid::parse_str(\"{uuid}\").unwrap(), 
                        transform:{transform},
                        rank: 0
                    }}],
                local_transforms: vec![
                    benthic_protocol::skeleton::Transform{{
                        name:\"Default\".to_string(), 
                        id: uuid::Uuid::parse_str(\"{uuid}\").unwrap(), 
                        transform:{local_transform},
                        rank: 0
                    }}],
                }})",
                n = format!("{:?}", name),
                parent = match &joint.parent {
                    Some(p) => format!("Some(benthic_protocol::skeleton::JointName::{:?})", p),
                    None => "None".to_string(),
                },
                children = children,
                uuid = Uuid::nil(),
                transform = transform_str,
                local_transform = local_transform_str,
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "benthic_protocol::skeleton::Skeleton {{
            joints: vec![{}].into_iter().collect(),
            root: vec![benthic_protocol::skeleton::JointName::Pelvis],
        }}",
        joints_code
    )
}
