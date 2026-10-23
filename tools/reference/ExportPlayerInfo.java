// Development-only original packet codecs. No Minecraft world, server or GPU.
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.*;
import net.minecraft.network.protocol.game.ClientboundPlayerInfoUpdatePacket.*;
import net.minecraft.network.chat.*;
import net.minecraft.world.level.GameType;
import net.minecraft.world.entity.player.ProfilePublicKey;
import com.mojang.authlib.*;
import com.mojang.authlib.properties.*;
import com.google.common.collect.ImmutableMultimap;
import com.google.gson.*;
import io.netty.buffer.Unpooled;
import java.nio.file.*;
import java.time.Instant;
import java.util.*;
import java.security.KeyPairGenerator;

public class ExportPlayerInfo {
 static JsonArray packets=new JsonArray();
 static UUID id(int n){return new UUID(1,n);}
 static <T> void packet(String label,int id,StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T value) {
  var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
  codec.encode(buffer,value);byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);buffer.release();
  var row=new JsonObject();row.addProperty("label",label);row.addProperty("id",id);row.addProperty("body",Base64.getEncoder().encodeToString(bytes));packets.add(row);
 }
 static void update(String label,EnumSet<Action> actions,List<Entry> entries)throws Exception {
  var packet=new ClientboundPlayerInfoUpdatePacket(actions,List.of());
  var field=ClientboundPlayerInfoUpdatePacket.class.getDeclaredField("entries");field.setAccessible(true);field.set(packet,entries);
  packet(label,0x46,ClientboundPlayerInfoUpdatePacket.STREAM_CODEC,packet);
 }
 static Entry entry(int n,String name,boolean listed,int latency,GameType mode,Component display,boolean hat,int order,RemoteChatSession.Data chat){
  var properties=new PropertyMap(ImmutableMultimap.of("textures",new Property("textures","c2tpbg==","signature"),"custom",new Property("custom","value")));
  return new Entry(id(n),new GameProfile(id(n),name,properties),listed,latency,mode,display,hat,order,chat);
 }
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var key=KeyPairGenerator.getInstance("RSA");key.initialize(2048);
  var chat=new RemoteChatSession.Data(id(99),new ProfilePublicKey.Data(Instant.ofEpochMilli(2000000000000L),key.generateKeyPair().getPublic(),new byte[]{1,2,3,4}));
  var alice=entry(1,"Alice",true,42,GameType.SURVIVAL,Component.literal("Alice display").withStyle(net.minecraft.ChatFormatting.GOLD),false,7,chat);
  var bob=entry(2,"bob",true,1200,GameType.SPECTATOR,null,true,-2,null);
  var hidden=entry(3,"Hidden",false,-1,GameType.CREATIVE,null,true,0,null);
  update("full",EnumSet.allOf(Action.class),List.of(alice,bob,hidden));
  for(var action:Action.values()) {
   var changed=entry(1,"Renamed",false,800,GameType.ADVENTURE,null,true,12,null);
   update(action.name(),EnumSet.of(action),List.of(changed));
  }
  packet("header_footer",0x7a,ClientboundTabListPacket.STREAM_CODEC,new ClientboundTabListPacket(Component.literal("Header\nSecond line").withStyle(net.minecraft.ChatFormatting.GREEN),Component.literal("Footer")));
  packet("clear_header_footer",0x7a,ClientboundTabListPacket.STREAM_CODEC,new ClientboundTabListPacket(Component.empty(),Component.empty()));
  packet("remove",0x45,ClientboundPlayerInfoRemovePacket.STREAM_CODEC,new ClientboundPlayerInfoRemovePacket(List.of(id(1),id(3))));
  var result=new JsonObject();result.addProperty("client_sha1","2dc72797acbc1b63fc16a11c4ac393605f453754");result.add("packets",packets);
  Files.writeString(Path.of(args[0]),new Gson().toJson(result));System.out.println("Exported "+packets.size()+" player info packets");
 }
}
