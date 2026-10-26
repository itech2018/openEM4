//! Lightweight scene entities for independent model identity and transforms.
//! EM4 source coordinates are Z-up; translation/yaw operate in that space.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct EntityId(pub u64);
#[derive(Clone,Copy,Debug)]
pub struct Transform {pub translation:[f32;3],pub yaw_degrees:f32,pub scale:f32}
impl Default for Transform{
 fn default()->Self{Self{translation:[0.0;3],yaw_degrees:0.0,scale:1.0}}
}
impl Transform{
 pub fn apply_position(&self,p:[f32;3])->[f32;3]{
  let (sin,cos)=self.yaw_degrees.to_radians().sin_cos();
  let x=p[0]*self.scale;let y=p[1]*self.scale;
  [cos*x-sin*y+self.translation[0],sin*x+cos*y+self.translation[1],p[2]*self.scale+self.translation[2]]
 }
 pub fn apply_normal(&self,n:[f32;3])->[f32;3]{
  let (sin,cos)=self.yaw_degrees.to_radians().sin_cos();
  [cos*n[0]-sin*n[1],sin*n[0]+cos*n[1],n[2]]
 }
 pub fn validate(&self)->Result<(),String>{
  if !self.translation.iter().all(|v|v.is_finite())||!self.yaw_degrees.is_finite()||
   !self.scale.is_finite()||self.scale<=0.0||self.scale>100.0{
   return Err("scene transform requires finite position/yaw and scale in (0,100]".into())
  }
  Ok(())
 }
}
#[derive(Clone,Debug)]
pub struct SceneEntity{pub id:EntityId,pub name:String,pub transform:Transform}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn identity_and_translation(){let t=Transform{translation:[2.,3.,4.],..Default::default()};assert_eq!(t.apply_position([1.,1.,1.]),[3.,4.,5.]);}
 #[test]fn yaw_rotates_without_moving_normal(){let t=Transform{yaw_degrees:90.,..Default::default()};let p=t.apply_position([1.,0.,0.]);assert!(p[0].abs()<1e-5&&(p[1]-1.).abs()<1e-5);}
 #[test]fn invalid_scale_is_rejected(){assert!(Transform{scale:0.,..Default::default()}.validate().is_err());}
}
