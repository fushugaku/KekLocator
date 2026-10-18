// @generated from the original DTCM parameter table.
use super::ParamDef;
pub const PARAMETERS: [ParamDef;20] = [
ParamDef { id:0, key:"dirty", label:"Dirty", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:true, cc:None },
ParamDef { id:1, key:"orbit", label:"Orbit", min:0.0f32, max:1.0f32, initial:0.20000000298023224f32, toggle:false, cc:Some(76) },
ParamDef { id:2, key:"reverb", label:"Reverb", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:false, cc:Some(75) },
ParamDef { id:3, key:"time", label:"Time", min:-96.0f32, max:24.0f32, initial:0.0f32, toggle:false, cc:Some(70) },
ParamDef { id:4, key:"cutoff", label:"Filter Cutoff", min:10.0f32, max:137.0f32, initial:73.0f32, toggle:false, cc:Some(72) },
ParamDef { id:5, key:"mod_speed", label:"Mod Speed", min:0.0f32, max:2.0f32, initial:0.5f32, toggle:false, cc:Some(73) },
ParamDef { id:6, key:"mod_depth", label:"Mod Depth", min:0.0f32, max:1.0f32, initial:0.25f32, toggle:false, cc:Some(74) },
ParamDef { id:7, key:"pitch", label:"Pitch Shift", min:-12.0f32, max:12.0f32, initial:12.0f32, toggle:false, cc:Some(79) },
ParamDef { id:8, key:"shimmer", label:"Shimmer", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:false, cc:Some(80) },
ParamDef { id:9, key:"feedback", label:"Feedback", min:0.0f32, max:1.2699999809265137f32, initial:0.0f32, toggle:false, cc:Some(78) },
ParamDef { id:10, key:"resonance", label:"Filter Q", min:0.0f32, max:1.0f32, initial:0.5f32, toggle:false, cc:Some(71) },
ParamDef { id:11, key:"ping_pong", label:"Ping Pong", min:-1.0f32, max:1.0f32, initial:0.0f32, toggle:false, cc:Some(81) },
ParamDef { id:12, key:"compression", label:"Compressor", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:false, cc:None },
ParamDef { id:13, key:"shimmer_mode", label:"Shimmer Mode", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:true, cc:None },
ParamDef { id:14, key:"filter_mode", label:"Filter Mode", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:true, cc:None },
ParamDef { id:15, key:"dub", label:"Dub", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:true, cc:None },
ParamDef { id:16, key:"dub_mix", label:"Dub Mix", min:0.0f32, max:1.0f32, initial:0.5f32, toggle:false, cc:None },
ParamDef { id:17, key:"shimmer_style", label:"Shimmer Style", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:true, cc:None },
ParamDef { id:18, key:"mix", label:"Dry/Wet", min:0.0f32, max:1.0f32, initial:0.5f32, toggle:false, cc:Some(77) },
ParamDef { id:19, key:"freeze", label:"Freeze", min:0.0f32, max:1.0f32, initial:0.0f32, toggle:true, cc:Some(82) },
];
