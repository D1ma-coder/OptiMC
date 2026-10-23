// CPU-only 26.2 clock manager and network packet reference; no game/GPU/server.
import com.google.gson.*;import java.nio.file.*;import java.util.*;
import net.minecraft.core.*;import net.minecraft.core.registries.*;import net.minecraft.world.clock.*;import net.minecraft.resources.*;import net.minecraft.network.*;import net.minecraft.network.codec.*;import net.minecraft.network.protocol.game.*;import net.minecraft.network.protocol.common.*;import net.minecraft.network.protocol.common.custom.*;
public class ExportWorldEnvironment {
 static RegistryAccess access;
 static <T> JsonObject packet(String label,int id,StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T value) {
  var b=new RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(),access);codec.encode(b,value);byte[] bytes=new byte[b.readableBytes()];b.readBytes(bytes);b.release();var row=new JsonObject();row.addProperty("label",label);row.addProperty("id",id);row.addProperty("body",Base64.getEncoder().encodeToString(bytes));return row;
 }
 static JsonArray clocks(net.minecraft.client.ClientClockManager manager)throws Exception {
  var f=manager.getClass().getDeclaredField("clocks");f.setAccessible(true);var rows=new JsonArray();
  for(var e:((Map<Holder<WorldClock>,?>)f.get(manager)).entrySet()){
   var row=new JsonObject();row.addProperty("id",access.lookupOrThrow(Registries.WORLD_CLOCK).getId(e.getKey().value()));var value=e.getValue();for(String name:new String[]{"totalTicks","partialTick","rate"}){var field=value.getClass().getDeclaredField(name);field.setAccessible(true);row.addProperty(name,(Number)field.get(value));}rows.add(row);
  }return rows;
 }
 public static void main(String[] args)throws Exception {
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
  var registry=new MappedRegistry<WorldClock>(Registries.WORLD_CLOCK,com.mojang.serialization.Lifecycle.stable());
  // Intentionally reverse the normal name order; holder IDs are server-owned.
  var end=registry.register(WorldClocks.THE_END,new WorldClock(),RegistrationInfo.BUILT_IN);var overworld=registry.register(WorldClocks.OVERWORLD,new WorldClock(),RegistrationInfo.BUILT_IN);registry.freeze();access=new RegistryAccess.ImmutableRegistryAccess(List.of(registry));
  var root=new JsonObject();var names=new JsonArray();names.add("minecraft:the_end");names.add("minecraft:overworld");root.add("registry",names);
  var manager=new net.minecraft.client.ClientClockManager();var updates=new JsonArray();
  long[] times={100,137,161,90,91};
  for(int i=0;i<times.length;i++){
   Map<Holder<WorldClock>,ClockNetworkState> clocks=new LinkedHashMap<>();
   if(i==0){clocks.put(end,new ClockNetworkState(1000,.25f,.5f));clocks.put(overworld,new ClockNetworkState(6000,.75f,1));}
   if(i==1)clocks.put(overworld,new ClockNetworkState(18000,.5f,0));
   if(i==3)clocks.put(end,new ClockNetworkState(-24000,.125f,-.25f));
   if(i==4)clocks.put(overworld,new ClockNetworkState(9007199254745999L,.875f,2));
   var row=packet("time_"+i,0x71,ClientboundSetTimePacket.STREAM_CODEC,new ClientboundSetTimePacket(times[i],clocks));manager.handleUpdates(times[i],clocks);row.addProperty("gameTime",times[i]);row.add("clocks",clocks(manager));updates.add(row);
  }root.add("times",updates);
  var packets=new JsonArray();
  packets.add(packet("spawn",0x61,ClientboundSetDefaultSpawnPositionPacket.STREAM_CODEC,new ClientboundSetDefaultSpawnPositionPacket(new net.minecraft.world.level.storage.LevelData.RespawnData(GlobalPos.of(net.minecraft.world.level.Level.OVERWORLD,new BlockPos(-31,-22,47)),37.5f,-12.25f))));
  packets.add(packet("center",0x5e,ClientboundSetChunkCacheCenterPacket.STREAM_CODEC,new ClientboundSetChunkCacheCenterPacket(-300,512)));
  packets.add(packet("radius",0x5f,ClientboundSetChunkCacheRadiusPacket.STREAM_CODEC,new ClientboundSetChunkCacheRadiusPacket(12)));
  packets.add(packet("brand_config",1,ClientboundCustomPayloadPacket.CONFIG_STREAM_CODEC,new ClientboundCustomPayloadPacket(new BrandPayload("Paper fixture 26.2"))));
  packets.add(packet("brand_play",0x18,ClientboundCustomPayloadPacket.GAMEPLAY_STREAM_CODEC,new ClientboundCustomPayloadPacket(new BrandPayload("Paper fixture 26.2"))));
  var modes=new JsonArray();
  for(int flags=0;flags<16;flags++)for(float value:new float[]{-3,-.5f,-.49f,0,.49f,.5f,1,1.5f,2,2.5f,3,3.49f,3.5f,99}) {
   var abilities=new net.minecraft.world.entity.player.Abilities();abilities.invulnerable=(flags&1)!=0;abilities.flying=(flags&2)!=0;abilities.mayfly=(flags&4)!=0;abilities.instabuild=(flags&8)!=0;
   var mode=net.minecraft.world.level.GameType.byId(net.minecraft.util.Mth.floor(value+.5f));mode.updatePlayerAbilities(abilities);
   var row=packet("game_mode",0x26,ClientboundGameEventPacket.STREAM_CODEC,new ClientboundGameEventPacket(ClientboundGameEventPacket.CHANGE_GAME_MODE,value));
   row.addProperty("initialFlags",flags);row.addProperty("mode",mode.getId());row.addProperty("flags",(abilities.invulnerable?1:0)|(abilities.flying?2:0)|(abilities.mayfly?4:0)|(abilities.instabuild?8:0));modes.add(row);
  }root.add("gamemodes",modes);
  var difficulties=new JsonArray();
  for(int id:new int[]{-129,-5,-1,0,1,2,3,4,127,128,255,256})for(boolean locked:new boolean[]{false,true}) {
   var d=net.minecraft.world.Difficulty.byId(id);
   var row=packet("difficulty",0x0a,ClientboundChangeDifficultyPacket.STREAM_CODEC,new ClientboundChangeDifficultyPacket(d,locked));
   row.addProperty("difficulty",d.getId());row.addProperty("locked",locked);difficulties.add(row);
  }
  root.add("difficulties",difficulties);
  var distances=new JsonArray();
  for(int distance:new int[]{-1,0,2,12,32,128,Integer.MAX_VALUE}) {
   var row=packet("simulation",0x6f,ClientboundSetSimulationDistancePacket.STREAM_CODEC,new ClientboundSetSimulationDistancePacket(distance));row.addProperty("distance",distance);distances.add(row);
  }
  root.add("distances",distances);
  packets.add(difficulties.get(7));packets.add(distances.get(3));
  packets.add(packet("block_ack",0x04,ClientboundBlockChangedAckPacket.STREAM_CODEC,new ClientboundBlockChangedAckPacket(37)));
  root.add("packets",packets);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
