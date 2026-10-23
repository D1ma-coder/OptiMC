// CPU-only protocol/geometry reference; never launches a client, server or GPU.
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.resources.RegistryOps;
import com.mojang.serialization.JsonOps;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.syncher.*;
import net.minecraft.network.protocol.game.*;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.decoration.ItemFrame;
import net.minecraft.world.item.*;
import net.minecraft.world.level.material.MapColor;
import net.minecraft.world.level.saveddata.maps.*;
import net.minecraft.world.phys.Vec3;
import io.netty.buffer.Unpooled;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;

public class ExportMaps {
 static JsonArray packets = new JsonArray();
 static <T> void packet(String label,int id,StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T value) {
  var buffer = new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
  codec.encode(buffer,value);byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);buffer.release();
  var row=new JsonObject();row.addProperty("label",label);row.addProperty("id",id);row.addProperty("body",Base64.getEncoder().encodeToString(bytes));packets.add(row);
 }
 @SuppressWarnings("unchecked") static <T> EntityDataAccessor<T> accessor(Class<?> type,String name) throws Exception {
  var field=type.getDeclaredField(name);field.setAccessible(true);return (EntityDataAccessor<T>)field.get(null);
 }
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var result=new JsonObject();result.addProperty("client_sha1","2dc72797acbc1b63fc16a11c4ac393605f453754");
  var serializers=new JsonObject();
  for(var field:EntityDataSerializers.class.getFields()) if(EntityDataSerializer.class.isAssignableFrom(field.getType()))
   serializers.addProperty(field.getName(),EntityDataSerializers.getSerializedId((EntityDataSerializer<?>)field.get(null)));
  result.add("serializers",serializers);
  var palette=new JsonArray();for(int i=0;i<256;i++)palette.add(MapColor.getColorFromPackedId(i));result.add("palette",palette);
  var item=accessor(ItemFrame.class,"DATA_ITEM");var rotation=accessor(ItemFrame.class,"DATA_ROTATION");
  var flags=accessor(Entity.class,"DATA_SHARED_FLAGS_ID");
  result.addProperty("item_index",item.id());result.addProperty("rotation_index",rotation.id());result.addProperty("flags_index",flags.id());
  // Bootstrap alone leaves 26.2 holder component prototypes unbound. Seed only
  // this fixture's common base; the wire codec serializes the MAP_ID patch only.
  ((Holder.Reference<Item>)BuiltInRegistries.ITEM.wrapAsHolder(Items.FILLED_MAP)).bindComponents(DataComponents.COMMON_ITEM_COMPONENTS);
  result.addProperty("fixture_item_base","COMMON_ITEM_COMPONENTS; only MAP_ID patch serialized");
  var stack=new ItemStack(Items.FILLED_MAP);stack.set(DataComponents.MAP_ID,new MapId(42));
  packet("spawn",1,ClientboundAddEntityPacket.STREAM_CODEC,new ClientboundAddEntityPacket(99,new UUID(1,2),0.5,64.5,0.03125,0,180,EntityTypes.ITEM_FRAME,Direction.NORTH.get3DDataValue(),Vec3.ZERO,180));
  packet("contents",0x63,ClientboundSetEntityDataPacket.STREAM_CODEC,new ClientboundSetEntityDataPacket(99,List.of(
   SynchedEntityData.DataValue.create(item,stack),SynchedEntityData.DataValue.create(rotation,1),SynchedEntityData.DataValue.create(flags,(byte)0))));
  var colors=new byte[128*128];for(int y=0;y<128;y++)for(int x=0;x<128;x++)colors[y*128+x]=(byte)(4+(x/16+y/16*8)%60*4+(x%4));
  var decoration=new MapDecoration(MapDecorationTypes.RED_MARKER,(byte)12,(byte)-17,(byte)3,Optional.of(Component.literal("Marker")));
  packet("map",0x33,ClientboundMapItemDataPacket.STREAM_CODEC,new ClientboundMapItemDataPacket(new MapId(42),(byte)2,false,List.of(decoration),new MapItemSavedData.MapPatch(0,0,128,128,colors)));
  packet("patch",0x33,ClientboundMapItemDataPacket.STREAM_CODEC,new ClientboundMapItemDataPacket(new MapId(42),(byte)2,true,(Collection<MapDecoration>)null,new MapItemSavedData.MapPatch(3,7,2,2,new byte[]{4,5,6,7})));
  packet("icons_clear",0x33,ClientboundMapItemDataPacket.STREAM_CODEC,new ClientboundMapItemDataPacket(new MapId(42),(byte)2,true,List.of(),null));
  packet("empty",0x63,ClientboundSetEntityDataPacket.STREAM_CODEC,new ClientboundSetEntityDataPacket(99,List.of(SynchedEntityData.DataValue.create(item,ItemStack.EMPTY))));
  result.add("packets",packets);
  var componentFixtures=new JsonArray();
  var registryOps=RegistryOps.create(JsonOps.INSTANCE,RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
  var supported=Set.of("map_id","max_stack_size","max_damage","damage","repair_cost","enchantable","rarity","map_post_processing","ominous_bottle_amplifier","map_color","dyed_color","custom_data","custom_name","item_name","map_decorations","intangible_projectile","lore","item_model","tooltip_style","note_block_sound","enchantment_glint_override","unbreakable","creative_slot_lock","hide_additional_tooltip","hide_tooltip");
  for(var type:BuiltInRegistries.DATA_COMPONENT_TYPE) {
   var key=BuiltInRegistries.DATA_COMPONENT_TYPE.getKey(type);
   if(!supported.contains(key.getPath()))continue;
   Object value=null;
   if(type.codec()!=null)for(String sample:List.of("{}","{\"value\":5}","{\"rgb\":5}","true","1","5","\"minecraft:stone\"","\"test\"","[]","{\"text\":\"Label\"}","[{\"text\":\"Lore\"}]","\"common\"","\"lock\"")) {
    try {var parsed=type.codec().parse(registryOps,JsonParser.parseString(sample)).result();if(parsed.isPresent()){value=parsed.get();break;}}catch(Exception ignored){}
   }
   if(value==null && key.getPath().equals("map_post_processing"))value=net.minecraft.world.item.component.MapPostProcessing.LOCK;
   if(value==null && key.getPath().equals("creative_slot_lock"))value=net.minecraft.util.Unit.INSTANCE;
   if(value==null)throw new IllegalStateException("No sample for supported component "+key);
   var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
   ((StreamCodec)type.streamCodec()).encode(buffer,value);
   byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);buffer.release();
   var row=new JsonObject();row.addProperty("name",key.toString());row.addProperty("id",BuiltInRegistries.DATA_COMPONENT_TYPE.getId(type));row.addProperty("body",Base64.getEncoder().encodeToString(bytes));componentFixtures.add(row);
  }
  result.add("components",componentFixtures);
  var transforms=new JsonArray();
  for(Direction direction:Direction.values())for(int r=0;r<8;r++)for(boolean invisible:List.of(false,true)){
   var pose=new PoseStack();pose.translate(direction.getStepX()*0.46875,direction.getStepY()*0.46875,direction.getStepZ()*0.46875);
   float pitch=direction.getAxis().isHorizontal()?0:-90*direction.getAxisDirection().getStep();float yaw=direction.getAxis().isHorizontal()?180-direction.toYRot():180;
   pose.mulPose(Axis.XP.rotationDegrees(pitch));pose.mulPose(Axis.YP.rotationDegrees(yaw));
   var frame=pose.last().pose().get(new float[16]);
   pose.translate(0,0,invisible?0.5f:0.4375f);pose.mulPose(Axis.ZP.rotationDegrees((r%4)*90));pose.mulPose(Axis.ZP.rotationDegrees(180));pose.scale(1f/128,1f/128,1f/128);pose.translate(-64,-64,-1);
   var row=new JsonObject();row.addProperty("direction",direction.get3DDataValue());row.addProperty("rotation",r);row.addProperty("invisible",invisible);
   var m=new JsonArray();for(float f:frame)m.add(f);row.add("frame",m);m=new JsonArray();for(float f:pose.last().pose().get(new float[16]))m.add(f);row.add("map",m);transforms.add(row);
  }
  result.add("transforms",transforms);Files.writeString(Path.of(args[0]),new Gson().toJson(result));
  System.out.println("Exported map/frame protocol, palette and "+transforms.size()+" transforms");
 }
}
