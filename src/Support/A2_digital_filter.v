//===============================================================================================================================
//
//    Copyright © 1996 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name         : Digital Filter
//
//    Author               : RTT
//    Creation Date        : 07/28/96
//    Version Number       : 1.0
//
//    Description          :   Digital filter
//                             Uses bi-directional Grey Code counter with saturation at end-points.
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED ``AS IS'' AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//    Notes for Use and Synthesis
//
//
//===============================================================================================================================
//
// NORMAL OPERATION -----------------------------------
//
//           __   :__   :__   :__   :__   :__   :__    __    __    __   :__   :__    __    __    __    __    __    __    __
//  clock __/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \
//                _______________________________________________       :     :
// source _______/:     :     :     :     :     : \\\\\\\\\\\\\\\\_________________________________________________________
//                : _______________________________________________     :     :
// catch  _________/    :     :     :     :     :                  \_______________________________________________________
//                      : _______________________________________________     :
// sample _______________/    :     :     :     :                       :\_________________________________________________
//        ____________________ _____ _____ _____ _____________________________ _____ _____ _____ __________________________
// filter _________000________X__1__X__3__X__2__X__6__________________________X__7__X__5__X__4__X_0________________________
//                                              ______________________________________________
// level  _____________________________________/                                              \____________________________
//
// ONE-CYCLE GLITCH =====================TWO CYCLE GLITCH =====================THREE CYCLE GLITCH================================
//
//           __   :__   :__   :__   :__   :__   :__    __    __    __    __   :__    __    __    __    __    __    __    __
//  clock __/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \
//                _____ :     :     :     _________                           _____________
// source _______/:    \_________________/:     :  \_________________________/:            \_______________________________
//                : _____     :           : ___________                       : ________________
// catch  _________/    :\_________________/    :      \_______________________/    :           \__________________________
//                      : _____                 :____________                       : ________________
// sample _______________/    :\________________/            \_______________________/                \____________________
//        ____________________ _____ _________________ _____ _____ _____ __________________ _____ _____ _____ _____ _____ _____
// filter _________000________X__1__X_0_______________X__1__X__3__X__1__X__0_______________X__1__X__3__X__2__X__3__X__1__X__0__
//
// level  _________________________________________________________________________________________________________________
//
// FASTEST OPERATION ============================================================================================================
//
//           __   :__   :__   :__   :__   :__   :__   :__   :__   :__   :__   :
//  clock __/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/  \__/
//                ___________________     :     :     :     _________________
// source _______/:     :     :     :\_____________________/      :     :
//                : _______________________     :     :       _______________
// catch  _________/    :     :     :     :\_________________/    :     :
//                      : _______________________     :     :     :     : ___
// sample _______________/    :     :     :     :\_______________________/
//        ____________________ _____ _____ _____ _____ _____ _____ _____ ____
// filter _________000________X__1__X__3__X__2__X__6__X__7__X__5__X__4__X__0_
//                            :     :     :     :________________________
// level  ______________________________________/                       :\____
//
// period of source is 8 x period of clock,  so clock rate must be 8 times faster than source
//===============================================================================================================================

module A2_digital_filter (
   output  wire   level,
   input   wire   source,
   input   wire   clock,
   input   wire   reset
   );

reg   [2:0] filter;                                   // 3-bit Gray code up/down counter
wire        sample;                                   // Physically must be an adjacent pair of flip-flops

A2_sync  i_sync (.q(sample), .d(source), .reset(reset), .clock(clock));

always @(posedge clock) begin
   if (reset)  filter  <=  3'b000;
   else casez  (filter) //***
      // Rising
      3'b000 : filter  <= sample ? 3'b001 : 3'b000;   // Saturates at '0'
      3'b001 : filter  <= sample ? 3'b011 : 3'b000;   // One cycle glitch catcher
      3'b011 : filter  <= sample ? 3'b010 : 3'b001;   // Two cycle glitch catcher
      3'b010 : filter  <= sample ? 3'b110 : 3'b011;   // Three cycle glitch catcher
      // Falling
      3'b110 : filter  <= sample ? 3'b110 : 3'b111;   // Saturates at '1'
      3'b111 : filter  <= sample ? 3'b110 : 3'b101;   // One cycle glitch catcher
      3'b101 : filter  <= sample ? 3'b111 : 3'b100;   // Two cycle glitch catcher
      3'b100 : filter  <= sample ? 3'b101 : 3'b000;   // Three cycle glitch catcher
      default : filter <= 3'b000; //***
      endcase
   end

assign   level = filter[2];                           // Waits for 4 consecutive samples before change

endmodule
