// CPU-only source sign packets and TextFieldHelper transitions.
import com.google.gson.*;import java.nio.file.*;import java.util.*;import java.util.concurrent.atomic.*;
import net.minecraft.network.*;import net.minecraft.network.protocol.game.*;import net.minecraft.core.BlockPos;
public class ExportSignEditor {
 static <T> String bytes(net.minecraft.network.codec.StreamCodec<? super FriendlyByteBuf,T> codec,T packet){var b=new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());codec.encode(b,packet);byte[] data=new byte[b.readableBytes()];b.readBytes(data);b.release();return Base64.getEncoder().encodeToString(data);}
 public static void main(String[] args)throws Exception {
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();var root=new JsonObject();var rows=new JsonArray();
  for(boolean front:new boolean[]{true,false})for(BlockPos pos:new BlockPos[]{new BlockPos(-31,-22,47),new BlockPos(0,0,0),new BlockPos(24,-45,-24),new BlockPos(33554431,2047,-33554432)})for(String[] lines:new String[][]{{"","","",""},{"Привет","  ","😀🌲","Четвёртая"},{"я".repeat(384),"😀".repeat(192),"x",""}}){
   var r=new JsonObject();r.addProperty("front",front);r.add("position",new Gson().toJsonTree(new int[]{pos.getX(),pos.getY(),pos.getZ()}));r.add("lines",new Gson().toJsonTree(lines));r.addProperty("open",bytes(ClientboundOpenSignEditorPacket.STREAM_CODEC,new ClientboundOpenSignEditorPacket(pos,front)));r.addProperty("update",bytes(ServerboundSignUpdatePacket.STREAM_CODEC,new ServerboundSignUpdatePacket(pos,front,lines[0],lines[1],lines[2],lines[3])));rows.add(r);
  }root.add("packets",rows);
  var value=new AtomicReference<String>("Hello 🌲 мир");var helper=new net.minecraft.client.gui.font.TextFieldHelper(value::get,value::set,()->"",s->{},s->s.codePointCount(0,s.length())<=12);var edits=new JsonArray();
  String[] ops={"open","left","insert","word_select","insert","home","delete","all","insert","end","left_select","backspace","insert"};String[] inputs={"","","!","","Русский","","","","中文😀","","","","0123456789012"};
  for(int i=0;i<ops.length;i++) {
   switch(ops[i]){case "left":helper.moveByChars(-1,false);break;case "insert":helper.insertText(inputs[i]);break;case "word_select":helper.moveByWords(-1,true);break;case "home":helper.setCursorToStart();break;case "delete":helper.removeCharsFromCursor(1);break;case "all":helper.selectAll();break;case "end":helper.setCursorToEnd();break;case "left_select":helper.moveByChars(-1,true);break;case "backspace":helper.removeCharsFromCursor(-1);break;}
   var r=new JsonObject();r.addProperty("op",ops[i]);r.addProperty("input",inputs[i]);r.addProperty("text",value.get());r.addProperty("cursor",helper.getCursorPos());r.addProperty("anchor",helper.getSelectionPos());edits.add(r);
  }root.add("edits",edits);
  var nbt=new JsonArray();
  var mixed=new net.minecraft.nbt.ListTag();mixed.add(net.minecraft.nbt.StringTag.valueOf("Front"));mixed.add(net.minecraft.nbt.IntTag.valueOf(7));
  var rich=new net.minecraft.nbt.CompoundTag();rich.putString("text","Rich");mixed.add(rich);
  var literal=new net.minecraft.nbt.CompoundTag();literal.putString("","literal key");mixed.add(literal);
  var strings=new net.minecraft.nbt.ListTag();strings.add(net.minecraft.nbt.StringTag.valueOf("a"));strings.add(net.minecraft.nbt.StringTag.valueOf(""));strings.add(net.minecraft.nbt.StringTag.valueOf("😀"));
  var wrapperList=new net.minecraft.nbt.ListTag();wrapperList.add(literal);
  var nested=new net.minecraft.nbt.ListTag();nested.add(strings);nested.add(net.minecraft.nbt.LongTag.valueOf(123));nested.add(new net.minecraft.nbt.ListTag());
  for(var tag:List.of(mixed,strings,wrapperList,nested,new net.minecraft.nbt.ListTag())) {
   var bytes=new java.io.ByteArrayOutputStream();net.minecraft.nbt.NbtIo.writeAnyTag(tag,new java.io.DataOutputStream(bytes));nbt.add(Base64.getEncoder().encodeToString(bytes.toByteArray()));
  }root.add("nbt",nbt);
Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
