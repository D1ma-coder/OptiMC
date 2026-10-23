// CPU-only codecs and transformations from the installed official 26.2 JAR.
import net.minecraft.SharedConstants;
import net.minecraft.ChatFormatting;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.block.entity.*;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.PlainSignBlock;
import net.minecraft.world.item.DyeColor;
import net.minecraft.network.*;
import net.minecraft.network.chat.*;
import net.minecraft.network.protocol.game.*;
import net.minecraft.nbt.*;
import net.minecraft.resources.RegistryOps;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.network.syncher.*;
import io.netty.buffer.Unpooled;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
public class ExportSigns {
 static void set(Object target,String name,Object value)throws Exception{var f=target.getClass().getDeclaredField(name);f.setAccessible(true);f.set(target,value);}
 static JsonObject part(net.minecraft.client.model.geom.ModelPart part)throws Exception{
  var out=new JsonObject();var transform=new JsonArray();for(float v:new float[]{part.x,part.y,part.z,part.xRot,part.yRot,part.zRot,part.xScale,part.yScale,part.zScale})transform.add(v);out.add("transform",transform);out.addProperty("visible",part.visible);out.addProperty("skip_draw",part.skipDraw);
  var cubes=part.getClass().getDeclaredField("cubes");cubes.setAccessible(true);var quads=new JsonArray();for(Object cube:(List<?>)cubes.get(part)){var polygons=cube.getClass().getDeclaredField("polygons");polygons.setAccessible(true);for(Object polygon:(Object[])polygons.get(cube)){var vertices=polygon.getClass().getDeclaredField("vertices");vertices.setAccessible(true);var quad=new JsonArray();for(Object vertex:(Object[])vertices.get(polygon)){var point=new JsonArray();for(String key:List.of("x","y","z","u","v"))point.add((Float)vertex.getClass().getMethod(key).invoke(vertex));quad.add(point);}quads.add(quad);}}out.add("quads",quads);
  var children=part.getClass().getDeclaredField("children");children.setAccessible(true);var nested=new JsonObject();for(var entry:((Map<String,net.minecraft.client.model.geom.ModelPart>)children.get(part)).entrySet())nested.add(entry.getKey(),part(entry.getValue()));out.add("children",nested);return out;
 }
 static JsonArray matrices=new JsonArray();
 static void matrix(String name,float angle,boolean front,org.joml.Matrix4fc value){var row=new JsonObject();row.addProperty("kind",name);row.addProperty("angle",angle);row.addProperty("front",front);var data=new JsonArray();for(float v:value.get(new float[16]))data.add(v);row.add("matrix",data);matrices.add(row);}
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var result=new JsonObject();result.addProperty("client_sha1","2dc72797acbc1b63fc16a11c4ac393605f453754");
  var registry=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
  var sign=new SignBlockEntity(new BlockPos(-31,-22,47),Blocks.OAK_SIGN.defaultBlockState());
  set(sign,"frontText",new SignText().setMessage(0,Component.literal("Привет").withStyle(ChatFormatting.BOLD)).setMessage(1,Component.literal("Front")).setColor(DyeColor.RED).setHasGlowingText(true));
  set(sign,"backText",new SignText().setMessage(0,Component.literal("Back")));
  var tag=sign.getUpdateTag(registry);var ctor=ClientboundBlockEntityDataPacket.class.getDeclaredConstructor(BlockPos.class,BlockEntityType.class,CompoundTag.class);ctor.setAccessible(true);var packet=ctor.newInstance(sign.getBlockPos(),sign.getType(),tag);
  var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),registry);ClientboundBlockEntityDataPacket.STREAM_CODEC.encode(buffer,packet);byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);buffer.release();result.addProperty("update",Base64.getEncoder().encodeToString(bytes));
  buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),registry);buffer.writeVarInt(1);buffer.writeByte(1<<4|15);buffer.writeShort(-22);buffer.writeVarInt(BuiltInRegistries.BLOCK_ENTITY_TYPE.getId(sign.getType()));buffer.writeNbt(tag);bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);buffer.release();result.addProperty("chunk_entities",Base64.getEncoder().encodeToString(bytes));
  var standing=Class.forName("net.minecraft.client.renderer.blockentity.StandingSignRenderer").getDeclaredMethod("textTransformation",PlainSignBlock.Attachment.class,float.class,boolean.class);standing.setAccessible(true);
  var hanging=Class.forName("net.minecraft.client.renderer.blockentity.HangingSignRenderer").getDeclaredMethod("textTransformation",float.class,boolean.class);hanging.setAccessible(true);
  for(int r=0;r<16;r++)for(boolean front:List.of(false,true)){float angle=r*22.5f;for(var attachment:PlainSignBlock.Attachment.values())matrix(attachment.name(),angle,front,((com.mojang.math.Transformation)standing.invoke(null,attachment,angle,front)).getMatrix());matrix("HANGING",angle,front,((com.mojang.math.Transformation)hanging.invoke(null,angle,front)).getMatrix());}
  result.add("matrices",matrices);var dyes=new JsonObject();for(var color:DyeColor.values())dyes.addProperty(color.getSerializedName(),color.getTextColor());result.add("dyes",dyes);
  var metadata=new JsonObject();for(String field:List.of("DATA_CLIENT_FLAGS","DATA_HEAD_POSE","DATA_BODY_POSE","DATA_LEFT_ARM_POSE","DATA_RIGHT_ARM_POSE","DATA_LEFT_LEG_POSE","DATA_RIGHT_LEG_POSE")){var f=ArmorStand.class.getDeclaredField(field);f.setAccessible(true);metadata.addProperty(field,((EntityDataAccessor<?>)f.get(null)).id());}result.add("armor_indices",metadata);
  var defaults=new JsonObject();for(String field:List.of("DEFAULT_HEAD_POSE","DEFAULT_BODY_POSE","DEFAULT_LEFT_ARM_POSE","DEFAULT_RIGHT_ARM_POSE","DEFAULT_LEFT_LEG_POSE","DEFAULT_RIGHT_LEG_POSE")){var pose=(Rotations)ArmorStand.class.getField(field).get(null);var array=new JsonArray();array.add(pose.x());array.add(pose.y());array.add(pose.z());defaults.add(field,array);}result.add("armor_defaults",defaults);
  var roots=net.minecraft.client.model.geom.LayerDefinitions.createRoots();
  result.add("armor_small",part(roots.get(net.minecraft.client.model.geom.ModelLayers.ARMOR_STAND_SMALL).bakeRoot()));
  result.add("armor_big",part(roots.get(net.minecraft.client.model.geom.ModelLayers.ARMOR_STAND).bakeRoot()));
  var samples=new JsonArray();
  for(int flags:new int[]{0,4,8,1,5,13,16}){
   var root=roots.get((flags&1)!=0?net.minecraft.client.model.geom.ModelLayers.ARMOR_STAND_SMALL:net.minecraft.client.model.geom.ModelLayers.ARMOR_STAND).bakeRoot();var model=new net.minecraft.client.model.object.armorstand.ArmorStandModel(root);var state=new net.minecraft.client.renderer.entity.state.ArmorStandRenderState();
   state.yRot=37;state.showArms=(flags&4)!=0;state.showBasePlate=(flags&8)==0;
   state.headPose=new Rotations(15,25,-5);state.bodyPose=new Rotations(10,-20,30);state.leftArmPose=new Rotations(-45,12,8);state.rightArmPose=new Rotations(30,15,-20);state.leftLegPose=new Rotations(9,3,-5);state.rightLegPose=new Rotations(-15,8,17);model.setupAnim(state);
   var sample=new JsonObject();sample.addProperty("flags",flags);sample.add("root",part(root));samples.add(sample);
  }result.add("armor_samples",samples);
  Files.writeString(Path.of(args[0]),new Gson().toJson(result));System.out.println("Exported signs and armor stand metadata");
 }
}
