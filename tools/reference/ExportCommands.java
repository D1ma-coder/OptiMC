// Development-only argument serializers and command graph codec reference.
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.game.ClientboundCommandsPacket;
import net.minecraft.commands.synchronization.ArgumentTypeInfo;
import net.minecraft.commands.arguments.GameProfileArgument;
import net.minecraft.commands.arguments.MessageArgument;
import net.minecraft.resources.Identifier;
import com.mojang.brigadier.builder.*;
import com.mojang.brigadier.tree.*;
import io.netty.buffer.Unpooled;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
public class ExportCommands {
 static String encoded(FriendlyByteBuf b){byte[] bytes=new byte[b.readableBytes()];b.readBytes(bytes);b.release();return Base64.getEncoder().encodeToString(bytes);}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var result=new JsonObject();var types=new JsonArray();
  for(var info:BuiltInRegistries.COMMAND_ARGUMENT_TYPE) {
   var name=BuiltInRegistries.COMMAND_ARGUMENT_TYPE.getKey(info);var input=new FriendlyByteBuf(Unpooled.buffer());
   if(Set.of("resource","resource_key","resource_or_tag","resource_or_tag_key","resource_selector").contains(name.getPath()))input.writeUtf("minecraft:block");else input.writeZero(32);
   var template=info.deserializeFromNetwork(input);input.release();var out=new FriendlyByteBuf(Unpooled.buffer());((ArgumentTypeInfo)info).serializeToNetwork(template,out);
   var row=new JsonObject();row.addProperty("id",BuiltInRegistries.COMMAND_ARGUMENT_TYPE.getId(info));row.addProperty("name",name.toString());row.addProperty("properties",encoded(out));types.add(row);
  }
  result.add("arguments",types);
  var root=new RootCommandNode<Object>();root.addChild(LiteralArgumentBuilder.<Object>literal("help").executes(c->0).build());
  var msg=LiteralArgumentBuilder.<Object>literal("msg").then(RequiredArgumentBuilder.<Object,GameProfileArgument.Result>argument("targets",GameProfileArgument.gameProfile()).suggests((c,b)->b.suggest("Alice").buildFuture()).then(RequiredArgumentBuilder.<Object,MessageArgument.Message>argument("message",MessageArgument.message()).executes(c->0))).build();root.addChild(msg);root.addChild(LiteralArgumentBuilder.<Object>literal("tell").redirect(msg).build());
  var inspector=new ClientboundCommandsPacket.NodeInspector<Object>() {
   public Identifier suggestionId(ArgumentCommandNode<Object,?> n){return n.getCustomSuggestions()==null?null:Identifier.withDefaultNamespace("ask_server");}
   public boolean isExecutable(CommandNode<Object> n){return n.getCommand()!=null;}
   public boolean isRestricted(CommandNode<Object> n){return false;}
  };
  var out=new FriendlyByteBuf(Unpooled.buffer());ClientboundCommandsPacket.STREAM_CODEC.encode(out,new ClientboundCommandsPacket(root,inspector));result.addProperty("graph",encoded(out));
  result.addProperty("client_sha1","2dc72797acbc1b63fc16a11c4ac393605f453754");Files.writeString(Path.of(args[0]),new Gson().toJson(result));System.out.println("Exported "+types.size()+" command argument codecs");
 }
}
