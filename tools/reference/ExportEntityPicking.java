import java.nio.file.*;
import java.util.*;
import java.lang.reflect.*;
import com.google.gson.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.registries.BuiltInRegistries;
public class ExportEntityPicking {
 static sun.misc.Unsafe unsafe()throws Exception{var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);return (sun.misc.Unsafe)f.get(null);}
 static void field(Object e,Class<?> owner,String name,Object value)throws Exception{var f=owner.getDeclaredField(name);f.setAccessible(true);f.set(e,value);}
 static Method declared(Class<?> clazz,String name,Class<?>...args)throws Exception{for(var c=clazz;c!=null;c=c.getSuperclass())try{var m=c.getDeclaredMethod(name,args);m.setAccessible(true);return m;}catch(NoSuchMethodException e){}throw new NoSuchMethodException(name);}
 static void define(net.minecraft.network.syncher.SynchedEntityData.Builder b,String name,Object value)throws Exception{var f=net.minecraft.world.entity.Entity.class.getDeclaredField(name);f.setAccessible(true);b.define((net.minecraft.network.syncher.EntityDataAccessor)f.get(null),value);}
 static JsonArray boxBits(net.minecraft.world.phys.AABB b){var a=new JsonArray();for(double v:new double[]{b.minX,b.minY,b.minZ,b.maxX,b.maxY,b.maxZ})a.add(Long.toUnsignedString(Double.doubleToRawLongBits(v),16));return a;}
 static JsonArray box(net.minecraft.world.phys.AABB b){var a=new JsonArray();for(double v:new double[]{b.minX,b.minY,b.minZ,b.maxX,b.maxY,b.maxZ})a.add(v);return a;}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var rows=new JsonArray();
  for(var type:BuiltInRegistries.ENTITY_TYPE) {
   var row=new JsonObject();Class<?> clazz=type.getBaseClass();
   var factoryField=type.getClass().getDeclaredField("factory");factoryField.setAccessible(true);
   try {
    var entity=((net.minecraft.world.entity.EntityType.EntityFactory<?>)factoryField.get(type)).create((net.minecraft.world.entity.EntityType)type,null);
    if(entity!=null)clazz=entity.getClass();
   } catch(Throwable e) {
    for(var frame:e.getStackTrace())if(frame.getMethodName().equals("<init>") && frame.getClassName().startsWith("net.minecraft.world.entity."))clazz=Class.forName(frame.getClassName());
   }
   if(BuiltInRegistries.ENTITY_TYPE.getKey(type).toString().equals("minecraft:player"))clazz=net.minecraft.world.entity.player.Player.class;
   row.addProperty("name",BuiltInRegistries.ENTITY_TYPE.getKey(type).toString());row.addProperty("class",clazz.getName());
   row.addProperty("living",net.minecraft.world.entity.LivingEntity.class.isAssignableFrom(clazz));
   for(String method:List.of("isPickable","canBePickedFromInside","getPickRadius"))row.addProperty(method,clazz.getMethod(method).getDeclaringClass().getName());
   row.addProperty("width",type.getDimensions().width());row.addProperty("height",type.getDimensions().height());
   var indices=new JsonObject();
   for(var c=clazz;c!=null;c=c.getSuperclass())for(var f:c.getDeclaredFields())if(Modifier.isStatic(f.getModifiers()) && f.getType()==net.minecraft.network.syncher.EntityDataAccessor.class){f.setAccessible(true);indices.addProperty(f.getName(),((net.minecraft.network.syncher.EntityDataAccessor<?>)f.get(null)).id());}
   row.add("indices",indices);
   var sizes=new JsonArray();var poseBoxes=new JsonArray();var poseBoxBits=new JsonArray();var babySizes=new JsonArray();
   try {
    var concrete=clazz==net.minecraft.world.entity.player.Player.class?net.minecraft.client.player.RemotePlayer.class:clazz;
    var entity=(net.minecraft.world.entity.Entity)unsafe().allocateInstance(concrete);
    field(entity,net.minecraft.world.entity.Entity.class,"type",type);
    field(entity,net.minecraft.world.entity.Entity.class,"position",net.minecraft.world.phys.Vec3.ZERO);
    field(entity,net.minecraft.world.entity.Entity.class,"dimensions",type.getDimensions());
    var builder=new net.minecraft.network.syncher.SynchedEntityData.Builder(entity);
    define(builder,"DATA_SHARED_FLAGS_ID",(byte)0);define(builder,"DATA_AIR_SUPPLY_ID",300);define(builder,"DATA_CUSTOM_NAME_VISIBLE",false);define(builder,"DATA_SILENT",false);define(builder,"DATA_NO_GRAVITY",false);define(builder,"DATA_POSE",net.minecraft.world.entity.Pose.STANDING);define(builder,"DATA_TICKS_FROZEN",0);
    declared(concrete,"defineSynchedData",net.minecraft.network.syncher.SynchedEntityData.Builder.class).invoke(entity,builder);
    try{define(builder,"DATA_CUSTOM_NAME",Optional.empty());}catch(IllegalArgumentException duplicate){}
    field(entity,net.minecraft.world.entity.Entity.class,"entityData",builder.build());
    for(var pose:net.minecraft.world.entity.Pose.values()) {
     try {var d=entity.getDimensions(pose);var a=new JsonArray();a.add(d.width());a.add(d.height());sizes.add(a);poseBoxes.add(box(d.makeBoundingBox(new net.minecraft.world.phys.Vec3(-31,-22,47))));poseBoxBits.add(boxBits(d.makeBoundingBox(new net.minecraft.world.phys.Vec3(-31,-22,47))));}catch(Throwable err){sizes.add(JsonNull.INSTANCE);poseBoxes.add(JsonNull.INSTANCE);poseBoxBits.add(JsonNull.INSTANCE);}
    }
    String babyField=indices.has("DATA_BABY_ID")?"DATA_BABY_ID":(row.get("name").getAsString().equals("minecraft:armor_stand")?"DATA_CLIENT_FLAGS":null);
    if(babyField!=null){
     Field accessor=null;for(var c=concrete;c!=null && accessor==null;c=c.getSuperclass())try{accessor=c.getDeclaredField(babyField);}catch(NoSuchFieldException ignored){}
     accessor.setAccessible(true);Object key=accessor.get(null);Object data=entity.getEntityData();var item=declared(data.getClass(),"getItem",net.minecraft.network.syncher.EntityDataAccessor.class).invoke(data,key);var valueField=item.getClass().getDeclaredField("value");valueField.setAccessible(true);valueField.set(item,babyField.equals("DATA_CLIENT_FLAGS")?(Object)(byte)1:(Object)true);
     for(var pose:net.minecraft.world.entity.Pose.values()){try{var d=entity.getDimensions(pose);var a=new JsonArray();a.add(d.width());a.add(d.height());babySizes.add(a);}catch(Throwable err){babySizes.add(JsonNull.INSTANCE);}}
    }
   } catch(Throwable err){row.addProperty("dimensions_error",err.toString());}
   row.add("poses",sizes);row.add("pose_boxes",poseBoxes);row.add("pose_box_bits",poseBoxBits);row.add("baby_poses",babySizes);
   rows.add(row);
  }
  var root=new JsonObject();root.add("types",rows);var poseNames=new JsonArray();for(var p:net.minecraft.world.entity.Pose.values())poseNames.add(p.name());root.add("pose_names",poseNames);
  var frame=unsafe().allocateInstance(net.minecraft.world.entity.decoration.ItemFrame.class);
  var frameBox=declared(frame.getClass(),"createBoundingBox",net.minecraft.core.BlockPos.class,net.minecraft.core.Direction.class,boolean.class);
  var frames=new JsonArray();
  for(int x:new int[]{-30000000,-31,0,30000000})for(var d:net.minecraft.core.Direction.values())for(boolean map:new boolean[]{false,true}){
   var row=new JsonObject();row.addProperty("x",x);row.addProperty("face",d.get3DDataValue());row.addProperty("map",map);row.add("box",box((net.minecraft.world.phys.AABB)frameBox.invoke(frame,new net.minecraft.core.BlockPos(x,-22,47),d,map)));frames.add(row);
  }
  root.add("frames",frames);
  var actions=new JsonArray();
  for(var hand:net.minecraft.world.InteractionHand.values())for(boolean secondary:new boolean[]{false,true}) {
   var packet=new net.minecraft.network.protocol.game.ServerboundInteractPacket(300,hand,new net.minecraft.world.phys.Vec3(1.125,-0.125,0.25),secondary);
   var b=new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());packet.STREAM_CODEC.encode(b,packet);var bytes=new byte[b.readableBytes()];b.readBytes(bytes);b.release();var row=new JsonObject();row.addProperty("hand",hand.ordinal());row.addProperty("secondary",secondary);row.addProperty("body",Base64.getEncoder().encodeToString(bytes));actions.add(row);
  }root.add("interactions",actions);
  var tagMap=new LinkedHashMap<net.minecraft.resources.Identifier,it.unimi.dsi.fastutil.ints.IntList>();
  tagMap.put(net.minecraft.resources.Identifier.parse("minecraft:redirectable_projectile"),new it.unimi.dsi.fastutil.ints.IntArrayList(new int[]{1,3,127}));
  var tagPacket=new net.minecraft.network.protocol.common.ClientboundUpdateTagsPacket(Map.of(net.minecraft.core.registries.Registries.ENTITY_TYPE,new net.minecraft.tags.TagNetworkSerialization.NetworkPayload(tagMap)));
  var tagBuffer=new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());tagPacket.STREAM_CODEC.encode(tagBuffer,tagPacket);var tagBytes=new byte[tagBuffer.readableBytes()];tagBuffer.readBytes(tagBytes);tagBuffer.release();root.addProperty("tags_packet",Base64.getEncoder().encodeToString(tagBytes));
  var clips=new JsonArray();var random=new Random(776);for(int i=0;i<1000;i++){
   double x=(random.nextDouble()-.5)*10,y=(random.nextDouble()-.5)*10,z=(random.nextDouble()-.5)*10;
   var b=new net.minecraft.world.phys.AABB(x,y,z,x+random.nextDouble()*3,y+random.nextDouble()*3,z+random.nextDouble()*3);
   var start=new net.minecraft.world.phys.Vec3(x+(random.nextDouble()-.5)*4,y+(random.nextDouble()-.5)*4,z+(random.nextDouble()-.5)*4);
   var end=new net.minecraft.world.phys.Vec3(x+(random.nextDouble()-.5)*4,y+(random.nextDouble()-.5)*4,z+(random.nextDouble()-.5)*4);
   var row=new JsonObject();row.add("box",box(b));var a=new JsonArray();for(double v:new double[]{start.x,start.y,start.z,end.x,end.y,end.z})a.add(v);row.add("ray",a);row.addProperty("inside",b.contains(start));b.clip(start,end).ifPresent(v->{var hit=new JsonArray();hit.add(v.x);hit.add(v.y);hit.add(v.z);row.add("hit",hit);});clips.add(row);
  }root.add("clips",clips);
  Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
