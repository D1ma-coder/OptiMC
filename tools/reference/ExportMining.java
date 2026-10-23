// CPU-only vanilla block parameters and input packet codecs. No level/server/GPU.
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
import net.minecraft.network.*;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.*;
import net.minecraft.world.*;
import net.minecraft.world.effect.*;
import net.minecraft.world.level.*;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.material.*;
import io.netty.buffer.Unpooled;
public class ExportMining {
 static <T> void packet(JsonArray rows,String label,int id,StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T packet){
  var b=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));try{codec.encode(b,packet);var bytes=new byte[b.readableBytes()];b.readBytes(bytes);var row=new JsonObject();row.addProperty("label",label);row.addProperty("id",id);row.addProperty("body",Base64.getEncoder().encodeToString(bytes));rows.add(row);}finally{b.release();}
 }
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var states=new JsonArray();
  for(var state:Block.BLOCK_STATE_REGISTRY){var row=new JsonObject();row.addProperty("state",Block.getId(state));row.addProperty("block",BuiltInRegistries.BLOCK.getId(state.getBlock()));row.addProperty("hardness",state.getDestroySpeed(EmptyBlockGetter.INSTANCE,BlockPos.ZERO));row.addProperty("tool",state.requiresCorrectToolForDrops());row.addProperty("air",state.isAir());var fluid=state.getFluidState();row.addProperty("water",fluid.getType()==Fluids.WATER||fluid.getType()==Fluids.FLOWING_WATER?fluid.getOwnHeight():0);states.add(row);}
  var packets=new JsonArray();for(var hand:InteractionHand.values())packet(packets,"use_"+hand,0x43,ServerboundUseItemPacket.STREAM_CODEC,new ServerboundUseItemPacket(hand,300,-173.25f,32.5f));
  for(var action:List.of(ServerboundPlayerActionPacket.Action.START_DESTROY_BLOCK,ServerboundPlayerActionPacket.Action.ABORT_DESTROY_BLOCK,ServerboundPlayerActionPacket.Action.STOP_DESTROY_BLOCK))for(var face:Direction.values())packet(packets,action.name()+"_"+face,0x29,ServerboundPlayerActionPacket.STREAM_CODEC,new ServerboundPlayerActionPacket(action,new BlockPos(-31,-22,47),face,action==ServerboundPlayerActionPacket.Action.ABORT_DESTROY_BLOCK?0:300));
  var effects=new JsonArray();packet(effects,"haste",0x84,ClientboundUpdateMobEffectPacket.STREAM_CODEC,new ClientboundUpdateMobEffectPacket(42,new MobEffectInstance(MobEffects.HASTE,100,2,false,true,true),false));packet(effects,"fatigue",0x84,ClientboundUpdateMobEffectPacket.STREAM_CODEC,new ClientboundUpdateMobEffectPacket(42,new MobEffectInstance(MobEffects.MINING_FATIGUE,-1,1,false,false,true),true));packet(effects,"remove",0x4e,ClientboundRemoveMobEffectPacket.STREAM_CODEC,new ClientboundRemoveMobEffectPacket(42,MobEffects.HASTE));
  var root=new JsonObject();root.add("states",states);root.add("packets",packets);root.add("effects",effects);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
