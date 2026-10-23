// Original chat codecs and acknowledgement tracker, CPU only.
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.*;
import net.minecraft.network.chat.*;
import net.minecraft.commands.arguments.ArgumentSignatures;
import io.netty.buffer.Unpooled;
import com.google.gson.*;
import java.nio.file.*;
import java.time.Instant;
import java.util.*;
public class ExportChat {
 static JsonArray packets=new JsonArray();
 static <T> void packet(String label,int id,StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T value){
  var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));codec.encode(buffer,value);
  byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);buffer.release();var row=new JsonObject();row.addProperty("label",label);row.addProperty("id",id);row.addProperty("body",Base64.getEncoder().encodeToString(bytes));packets.add(row);
 }
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var at=Instant.ofEpochMilli(2000000000000L);var tracker=new LastSeenMessagesTracker(20);
  packet("chat",9,ServerboundChatPacket.STREAM_CODEC,new ServerboundChatPacket("Привет 😀",at,1234,null,tracker.generateAndApplyUpdate().update()));
  packet("command",7,ServerboundChatCommandPacket.STREAM_CODEC,new ServerboundChatCommandPacket("help"));
  packet("signed_command_unsigned_profile",8,ServerboundChatCommandSignedPacket.STREAM_CODEC,new ServerboundChatCommandSignedPacket("say Привет",at,1234,ArgumentSignatures.EMPTY,tracker.generateAndApplyUpdate().update()));
  packet("request",15,ServerboundCommandSuggestionPacket.STREAM_CODEC,new ServerboundCommandSuggestionPacket(42,"/msg Al"));
  packet("suggestions",15,ClientboundCommandSuggestionsPacket.STREAM_CODEC,new ClientboundCommandSuggestionsPacket(42,5,2,List.of(new ClientboundCommandSuggestionsPacket.Entry("Alice",Optional.of(Component.literal("Tooltip"))),new ClientboundCommandSuggestionsPacket.Entry("Alex",Optional.empty()))));
  packet("custom_add",23,ClientboundCustomChatCompletionsPacket.STREAM_CODEC,new ClientboundCustomChatCompletionsPacket(ClientboundCustomChatCompletionsPacket.Action.ADD,List.of("Alice","Alex")));
  packet("custom_remove",23,ClientboundCustomChatCompletionsPacket.STREAM_CODEC,new ClientboundCustomChatCompletionsPacket(ClientboundCustomChatCompletionsPacket.Action.REMOVE,List.of("Alex")));
  packet("custom_set",23,ClientboundCustomChatCompletionsPacket.STREAM_CODEC,new ClientboundCustomChatCompletionsPacket(ClientboundCustomChatCompletionsPacket.Action.SET,List.of("Bob")));
  var decoration=ChatTypeDecoration.withSender("chat.type.text");var bound=new ChatType.Bound(Holder.direct(new ChatType(decoration,decoration)),Component.literal("Alice"),Optional.empty());
  var body=new SignedMessageBody.Packed("Hello",at,1234,new LastSeenMessages.Packed(List.of()));
  packet("player_unsigned",65,ClientboundPlayerChatPacket.STREAM_CODEC,new ClientboundPlayerChatPacket(0,new UUID(1,1),0,null,body,null,FilterMask.PASS_THROUGH,bound));
  byte[] sigBytes=new byte[256];Arrays.fill(sigBytes,(byte)7);var sig=new MessageSignature(sigBytes);
  packet("player_signed",65,ClientboundPlayerChatPacket.STREAM_CODEC,new ClientboundPlayerChatPacket(1,new UUID(1,1),1,sig,body,Component.literal("Unsigned override"),FilterMask.PASS_THROUGH,bound));
  packet("disguised",33,ClientboundDisguisedChatPacket.STREAM_CODEC,new ClientboundDisguisedChatPacket(Component.literal("Disguised"),bound));
  packet("filtered",65,ClientboundPlayerChatPacket.STREAM_CODEC,new ClientboundPlayerChatPacket(2,new UUID(1,1),2,sig,body,null,FilterMask.FULLY_FILTERED,bound));
  packet("delete",31,ClientboundDeleteChatPacket.STREAM_CODEC,new ClientboundDeleteChatPacket(new MessageSignature.Packed(sig)));
  var updates=new JsonArray();
  for(int i=0;i<30;i++) {
   byte[] bytes=new byte[256];Arrays.fill(bytes,(byte)(i*11));var signature=new MessageSignature(bytes);boolean shown=i%3!=0;
   tracker.addPending(signature,shown);if(i%7==0)tracker.ignorePending(signature);
   if(i%5==4) {var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));tracker.generateAndApplyUpdate().update().write(buffer);byte[] wire=new byte[buffer.readableBytes()];buffer.readBytes(wire);buffer.release();updates.add(Base64.getEncoder().encodeToString(wire));}
  }
  var result=new JsonObject();result.addProperty("client_sha1","2dc72797acbc1b63fc16a11c4ac393605f453754");result.add("packets",packets);result.add("last_seen_updates",updates);Files.writeString(Path.of(args[0]),new Gson().toJson(result));System.out.println("Exported chat fixtures");
 }
}
